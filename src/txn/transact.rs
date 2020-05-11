use log::*;

use crate::{
    sync::{atomics::AtomicBox, treiber::TreiberStack},
    table::prelude::*,
};

use std::{
    sync::atomic::{AtomicU64, Ordering},
    thread,
};
use thread::ThreadId;

use super::readset::ReadSet;
use super::utils;
use crate::sync::ttas::TTas;
use std::cell::RefCell;
use std::{
    borrow::{Borrow, BorrowMut},
    time::Duration,
};
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, AtomicPtr},
        Arc,
    },
};

use crate::txn::conflicts::ConflictManager;
use crate::txn::vars::TVar;
use crate::txn::version::Version;
use crate::txn::writeset::WriteSet;
use lazy_static::*;
use std::any::Any;

#[derive(Clone)]
pub enum TransactionConcurrency {
    Optimistic,
    Pessimistic,
}

#[derive(Clone)]
pub enum TransactionIsolation {
    ///
    /// [TransactionIsolation::ReadCommitted] isolation level means that always a committed value will be
    /// provided for read operations. Values are always read from in-memory cache every time a
    /// value is accessed. In other words, if the same key is accessed more than once within the
    /// same transaction, it may have different value every time since global cache memory
    /// may be updated concurrently by other threads.
    ReadCommitted,
    ///
    /// [TransactionIsolation::RepeatableRead] isolation level means that if a value was read once within transaction,
    /// then all consecutive reads will provide the same in-transaction value. With this isolation
    /// level accessed values are stored within in-transaction memory, so consecutive access to
    /// the same key within the same transaction will always return the value that was previously
    /// read or updated within this transaction. If concurrency is
    /// [TransactionConcurrency::Pessimistic], then a lock on the key will be acquired
    /// prior to accessing the value.
    RepeatableRead,
    ///
    /// [TransactionIsolation::Serializable] isolation level means that all transactions occur in a completely isolated fashion,
    /// as if all transactions in the system had executed serially, one after the other. Read access
    /// with this level happens the same way as with [TransactionIsolation::RepeatableRead] level.
    /// However, in  [TransactionConcurrency::Optimistic] mode, if some transactions cannot be
    /// serially isolated from each other, then one winner will be picked and the other
    /// transactions in conflict will result with abort.
    Serializable,
}

#[derive(Debug, Clone)]
pub enum TransactionState {
    Active,
    Preparing,
    Prepared,
    MarkedRollback,
    Committing,
    Committed,
    RollingBack,
    RolledBack,
    Unknown,
    Suspended,
}

impl Default for TransactionState {
    fn default() -> Self {
        TransactionState::Unknown
    }
}

#[derive(Clone)]
pub struct Txn {
    /// Id of the transaction
    txid: u64,

    // NOTE: NonZeroU64 is std lib thread id interpret. Wait for the feature flag removal.
    /// Id of the thread in which this transaction started.
    tid: ThreadId,

    /// Txn isolation level
    pub(crate) iso: TransactionIsolation,

    /// Txn concurrency level
    pub(crate) cc: TransactionConcurrency,

    /// Txn state
    pub(crate) state: Arc<AtomicBox<TransactionState>>,

    /// Txn timeout
    ///
    /// * Gets timeout value in milliseconds for this transaction.
    timeout: usize,

    /// If transaction was marked as rollback-only.
    rollback_only: Arc<AtomicBool>,

    /// Label of the transaction
    label: String,
}

impl Txn {
    ///
    /// Initiate transaction with given closure.
    pub fn begin<F, R>(&self, mut f: F) -> R
    where
        F: FnMut(&mut Txn) -> R,
        R: 'static + Any + Clone + Send + Sync,
    {
        let r = loop {
            info!("tx_begin_read::txid::{}", self.txid);

            let me = self.clone();
            Self::set_local(me);

            // Refurbish
            let mut me = Self::get_local();
            me.on_start();

            /////////////////////////
            let res = f(&mut me);

            if me.on_validate::<R>() && me.commit() {
                me.on_commit::<R>();
                break res;
            }
            /////////////////////////

            me.on_abort::<R>();
        };

        r
    }

    ///
    /// Read initiator to the scratchpad from transactional variables.
    pub fn read<T: Send + Sync + Any + Clone>(&self, var: &TVar<T>) -> T {
        var.open_read()
    }

    ///
    /// Write back initiator for given transactional variables.
    pub fn write<T: Send + Sync + Any + Clone>(&mut self, var: &mut TVar<T>, value: T) -> T {
        var.open_write(value)
    }

    /// Modify the transaction associated with the current thread such that the
    /// only possible outcome of the transaction is to roll back the
    /// transaction.
    pub fn set_rollback_only(&mut self, flag: bool) {
        self.rollback_only.swap(flag, Ordering::SeqCst);
    }

    /// Commits this transaction by initiating two-phase-commit process.
    pub fn commit(&self) -> bool {
        self.state.replace_with(|_| TransactionState::Committed);
        true
    }

    /// Ends the transaction. Transaction will be rolled back if it has not been committed.
    pub fn close(&self) {
        todo!()
    }

    /// Rolls back this transaction.
    /// It's allowed to roll back transaction from any thread at any time.
    pub fn rollback(&self) {
        self.state
            .replace_with(|_| TransactionState::MarkedRollback);
    }

    /// Resume a transaction if it was previously suspended.
    /// Supported only for optimistic transactions.
    pub fn resume(&self) {
        match self.cc {
            TransactionConcurrency::Optimistic => {
                self.state.replace_with(|_| TransactionState::Active);
            }
            _ => {}
        }
    }

    /// Suspends a transaction. It could be resumed later.
    /// Supported only for optimistic transactions.
    pub fn suspend(&self) {
        match self.cc {
            TransactionConcurrency::Optimistic => {
                self.state.replace_with(|_| TransactionState::Suspended);
            }
            _ => {}
        }
    }

    ///
    /// Internal stage to update in-flight rollback
    pub(crate) fn rolling_back(&self) {
        self.state.replace_with(|_| TransactionState::RollingBack);
    }

    ///
    /// Internal stage to finalize rollback
    pub(crate) fn rolled_back(&self) {
        self.state.replace_with(|_| TransactionState::RolledBack);
    }

    ///
    /// Set the transaction going.
    /// Callback that will run before everything starts
    fn on_start(&self) {
        TxnManager::set_rts();
        self.state.replace_with(|_| TransactionState::Active);
    }

    ///
    /// Validates a transaction.
    /// Call this code when a transaction must decide whether it can commit.
    fn on_validate<T: 'static + Any + Clone + Send + Sync>(&self) -> bool {
        let mut ws = WriteSet::local();
        let rs = ReadSet::local();

        // TODO: Nanos or millis? Millis was the intention.
        if !ws.try_lock::<T>(Duration::from_millis(self.timeout as u64)) {
            // TODO: Can't acquire lock, write some good message here.
            // dbg!("Can't acquire lock");
            return false;
        }

        for x in rs.get_all_versions().iter().cloned() {
            let v: TVar<T> = utils::version_to_dest(x);

            if v.is_locked() && !v.is_writer_held_by_current_thread() {
                // TODO: MSG: Currently locked
                // dbg!("Currently locked");
                return false;
            }

            if !v.validate() {
                // TODO: MSG: Can't validate
                // dbg!("Can't validate");
                return false;
            }
        }

        true
    }

    ///
    /// Finalizing the commit and flush the write-backs to the main memory
    fn on_commit<T: Any + Clone + Send + Sync>(&mut self) {
        if !ConflictManager::check::<T>(&self.iso) {
            self.on_abort::<T>();
        }

        let mut ws = WriteSet::local();
        let mut rs = ReadSet::local();

        // TODO: MSG:
        // dbg!("Updating ws");

        TxnManager::set_wts();

        // TODO: MSG:
        // dbg!("Updated ws");

        let w_ts = TxnManager::rts();

        // TODO: MSG:
        // dbg!("Get write TS");

        for (k, source) in ws.get_all::<T>().iter_mut() {
            // let mut dest: T = k.open_read();
            k.data = Arc::new(source.clone());
            k.set_stamp(w_ts);
            info!("Enqueued writes are written");
        }

        ws.unlock::<T>();
        ws.clear::<T>();
        rs.clear();
    }

    pub(crate) fn on_abort<T: Clone + Send + Sync>(&self) {
        let mut ws = WriteSet::local();
        let mut rs = ReadSet::local();

        // TODO: MSG
        // dbg!("ON ABORT");

        TxnManager::set_rts();

        ws.clear::<T>();
        rs.clear();
    }

    /// Sets tlocal txn.
    pub(crate) fn set_local(ntxn: Txn) {
        TXN.with(|txn| {
            let mut txn = txn.borrow_mut();
            *txn = ntxn;
        })
    }

    /// Gets tlocal txn.
    pub fn get_local() -> Txn {
        // TODO: not sure
        TXN.with(|tx| tx.borrow().clone())
    }

    pub(crate) fn get_id(&self) -> u64 {
        self.txid
    }
}

impl Default for Txn {
    #[cfg_attr(miri, ignore)]
    fn default() -> Self {
        Self {
            txid: 0,
            tid: thread::current().id(),
            iso: TransactionIsolation::ReadCommitted,
            cc: TransactionConcurrency::Optimistic,
            state: Arc::new(AtomicBox::new(TransactionState::default())),
            timeout: 0,
            rollback_only: Arc::new(AtomicBool::default()),
            label: "default".into(),
        }
    }
}

thread_local! {
    static LOCAL_VC: RefCell<u64> = RefCell::new(0_u64);
    static TXN: RefCell<Txn> = RefCell::new(Txn::default());
}

lazy_static! {
    /// Global queues of transaction deltas.
    pub(crate) static ref GLOBAL_DELTAS: Arc<TTas<Vec<Version>>> = Arc::new(TTas::new(Vec::new()));
    /// TVar ids across all txns in the tx manager
    pub(crate) static ref GLOBAL_TVAR: Arc<AtomicU64> = Arc::new(AtomicU64::default());
    /// Version clock across all transactions
    pub(crate) static ref GLOBAL_VCLOCK: Arc<AtomicU64> = Arc::new(AtomicU64::default());
}

// Management layer
pub struct TxnManager {
    pub(crate) txid: Arc<AtomicU64>,
}

impl TxnManager {
    ///
    /// Instantiate transaction manager
    pub fn manager() -> Arc<TxnManager> {
        Arc::new(TxnManager {
            txid: Arc::new(AtomicU64::new(GLOBAL_VCLOCK.load(Ordering::SeqCst))),
        })
    }

    ///
    /// VC management: Sets read timestamp for the ongoing txn
    pub(crate) fn set_rts() {
        LOCAL_VC.with(|lvc| {
            let mut lvc = lvc.borrow_mut();
            *lvc = GLOBAL_VCLOCK.load(Ordering::SeqCst);
        });
    }

    ///
    /// VC management: Reads read timestamp for the ongoing txn
    pub(crate) fn rts() -> u64 {
        LOCAL_VC.with(|lvc| *lvc.borrow())
    }

    ///
    /// VC management: Sets write timestamp for the ongoing txn
    pub(crate) fn set_wts() {
        LOCAL_VC.with(|lvc| {
            let mut lvc = lvc.borrow_mut();
            *lvc = GLOBAL_VCLOCK
                .fetch_add(1, Ordering::SeqCst)
                .saturating_add(1);
        })
    }

    ///
    /// Dispense a new TVar ID
    pub(crate) fn dispense_tvar_id() -> u64 {
        GLOBAL_TVAR.fetch_add(1, Ordering::SeqCst).saturating_add(1)
    }

    ///
    /// Get latest dispensed TVar ID
    pub(crate) fn latest_tvar_id() -> u64 {
        GLOBAL_TVAR.fetch_add(1, Ordering::SeqCst)
    }

    ///
    /// Starts transaction with specified isolation, concurrency, timeout, invalidation flag,
    /// and number of participating entries.
    ///
    /// # Arguments
    /// * `cc`
    /// * `iso`
    /// * `timeout`: Timeout
    /// * `tx_size`: Number of entries participating in transaction (may be approximate).
    pub fn txn_build(
        &self,
        cc: TransactionConcurrency,
        iso: TransactionIsolation,
        timeout: usize,
        _tx_size: usize,
        label: String,
    ) -> Txn {
        // Timestamp and TX id are different concepts.
        let cur_txid: u64 = self.txid.fetch_add(1, Ordering::SeqCst) + 1;

        match (&iso, &cc) {
            (TransactionIsolation::ReadCommitted, TransactionConcurrency::Optimistic) => {
                todo!("OCC, with Read Committed, hasn't been implemented.");
            }
            (_, TransactionConcurrency::Pessimistic) => {
                todo!("PCC, with all isolation levels, hasn't been implemented.");
            }
            _ => {}
        }

        Txn {
            txid: cur_txid,              //
            tid: thread::current().id(), // Reset to thread id afterwards.
            iso,
            cc,
            state: Arc::new(AtomicBox::new(TransactionState::default())),
            timeout,
            rollback_only: Arc::new(AtomicBool::default()),
            label,
        }
    }
}

#[cfg(test)]
mod txn_tests {
    use super::*;

    #[test]
    #[ignore]
    fn txn_optimistic_read_committed() {
        let data = 100_usize;

        let txn = TxnManager::manager().txn_build(
            TransactionConcurrency::Optimistic,
            TransactionIsolation::ReadCommitted,
            100_usize,
            1_usize,
            "txn_optimistic_read_committed".into(),
        );

        let mut threads = vec![];
        let tvar = TVar::new(data);

        for thread_no in 0..2 {
            let txn = txn.clone();
            let mut tvar = tvar.clone();

            let t = std::thread::Builder::new()
                .name(format!("t_{}", thread_no))
                .spawn(move || {
                    if thread_no == 0 {
                        // Streamliner thread
                        *tvar = txn.begin(|t| {
                            let x = t.read(&tvar);
                            assert_eq!(x, 100);

                            thread::sleep(Duration::from_millis(300));

                            let x = t.read(&tvar);
                            assert_eq!(x, 123_000);
                            x
                        });
                    } else {
                        // Interceptor thread
                        *tvar = txn.begin(|t| {
                            thread::sleep(Duration::from_millis(100));

                            let mut x = t.read(&tvar);
                            assert_eq!(x, 100);

                            x = 123_000;
                            t.write(&mut tvar, x);

                            thread::sleep(Duration::from_millis(100));

                            x
                        });
                    }
                })
                .unwrap();

            threads.push(t);
        }

        for t in threads.into_iter() {
            t.join().unwrap();
        }
    }

    #[test]
    fn txn_optimistic_repeatable_read() {
        let data = 100_usize;

        let txn = TxnManager::manager().txn_build(
            TransactionConcurrency::Optimistic,
            TransactionIsolation::RepeatableRead,
            100_usize,
            1_usize,
            "txn_optimistic_repetable_read".into(),
        );

        let mut threads = vec![];
        let tvar = TVar::new(data);

        for thread_no in 0..2 {
            let txn = txn.clone();
            let mut tvar = tvar.clone();

            let t = std::thread::Builder::new()
                .name(format!("t_{}", thread_no))
                .spawn(move || {
                    if thread_no == 0 {
                        // Streamliner thread
                        txn.begin(|t| {
                            let x = t.read(&tvar);
                            assert_eq!(x, 100);

                            thread::sleep(Duration::from_millis(300));

                            let x = t.read(&tvar);
                            assert_eq!(x, 100);
                        })
                    } else {
                        // Interceptor thread
                        txn.begin(|t| {
                            thread::sleep(Duration::from_millis(100));

                            let mut x = t.read(&tvar);
                            assert_eq!(x, 100);

                            x = 123_000;
                            t.write(&mut tvar, x);

                            thread::sleep(Duration::from_millis(100));
                        })
                    }
                })
                .unwrap();

            threads.push(t);
        }

        for t in threads.into_iter() {
            t.join().unwrap();
        }
    }

    #[test]
    fn txn_optimistic_serializable() {
        let data = 100_usize;

        let txn = TxnManager::manager().txn_build(
            TransactionConcurrency::Optimistic,
            TransactionIsolation::RepeatableRead,
            100_usize,
            1_usize,
            "txn_optimistic_serializable".into(),
        );

        let mut threads = vec![];
        let tvar = TVar::new(data);

        for thread_no in 0..100 {
            let txn = txn.clone();
            let mut tvar = tvar.clone();

            let t = std::thread::Builder::new()
                .name(format!("t_{}", thread_no))
                .spawn(move || {
                    if thread_no % 2 == 0 {
                        // Streamliner thread
                        *tvar = txn.begin(|t| {
                            let x = t.read(&tvar);
                            assert_eq!(x, 100);

                            thread::sleep(Duration::from_millis(300));

                            let mut x = t.read(&tvar);
                            assert_eq!(x, 100);

                            x = 1453;
                            t.write(&mut tvar, x);

                            t.read(&tvar)
                        });
                    } else {
                        // Interceptor thread
                        *tvar = txn.begin(|t| {
                            thread::sleep(Duration::from_millis(100));

                            let mut x = t.read(&tvar);
                            assert_eq!(x, 100);

                            x = 123_000;
                            t.write(&mut tvar, x);

                            thread::sleep(Duration::from_millis(100));
                            x
                        });
                    }
                })
                .unwrap();

            threads.push(t);
        }

        for t in threads.into_iter() {
            // TODO: Write skews can make this fail. In snapshot mode.
            let _ = t.join();
        }
    }
}
