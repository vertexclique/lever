use std::{
    borrow::Borrow,
    cell::UnsafeCell,
    marker::PhantomData as marker,
    ops::{Deref, DerefMut},
    sync::{
        atomic::{self, Ordering},
        Arc,
    },
    time::Duration,
};

use super::{
    readset::ReadSet,
    transact::{TransactionState, Txn, TxnManager},
};

use super::version::*;
use log::*;

use parking_lot::*;

use super::utils;

use crate::txn::transact::TransactionConcurrency;
use crate::txn::writeset::WriteSet;
use std::alloc::{dealloc, Layout};
use std::any::Any;

///
/// Transactional variable
#[derive(Clone)]
pub struct TVar<T>
where
    T: Clone + Any + Send + Sync,
{
    pub(crate) data: Var,
    pub(crate) lock: Arc<ReentrantMutex<bool>>,
    /// TVar ID
    pub(crate) id: u64,
    /// R/W Timestamp
    pub(crate) stamp: u64,
    /// Revision of last modification on this key.
    pub(crate) modrev: u64,
    timeout: usize,
    marker: marker<T>,
}

impl<T> TVar<T>
where
    T: Clone + Any + Send + Sync,
{
    ///
    /// Instantiates transactional variable for later use in a transaction.
    pub fn new(data: T) -> Self {
        TVar {
            data: Arc::new(data),
            lock: Arc::new(ReentrantMutex::new(true)),
            id: TxnManager::dispense_tvar_id(),
            stamp: TxnManager::rts(),
            modrev: TxnManager::rts(),
            timeout: super::constants::DEFAULT_TX_TIMEOUT,
            marker,
        }
    }

    ///
    /// New transactional variable with overridden timeout for overriding timeout for specific
    /// transactional variable.
    ///
    /// Highly discouraged for the daily use unless you have various code paths that can
    /// interfere over the variable that you instantiate.
    pub fn new_with_timeout(data: T, timeout: usize) -> Self {
        TVar {
            data: Arc::new(data),
            lock: Arc::new(ReentrantMutex::new(true)),
            id: TxnManager::dispense_tvar_id(),
            stamp: TxnManager::rts(),
            modrev: TxnManager::rts(),
            timeout,
            marker,
        }
    }

    pub(crate) fn set_stamp(&mut self, stamp: u64) {
        self.stamp = stamp;
    }

    pub(crate) fn set_mod_rev(&mut self, modrev: u64) {
        self.modrev = modrev;
    }

    ///
    /// Get's the underlying data for the transactional variable.
    ///
    /// Beware that this will not give correct results any given point
    /// in time during the course of execution of a transaction.
    pub fn get_data(&self) -> T {
        let val = self.data.clone();

        (&*val as &dyn Any)
            .downcast_ref::<T>()
            .expect("Only tx vars are allowed for values.")
            .clone()
    }

    pub(crate) fn open_read(&self) -> T {
        let rs = ReadSet::local();
        let txn = Txn::get_local();
        let state: &TransactionState = &*txn.state.get();

        match state {
            TransactionState::Committed | TransactionState::Unknown => self.get_data(),
            TransactionState::Active => {
                let ws = WriteSet::local();

                let scratch = ws.get_by_stamp::<T>(self.stamp);

                if scratch.is_none() {
                    if self.is_locked() {
                        // TODO: throw abort
                        txn.rollback();
                        // panic!("READ: You can't lock and still continue processing");
                    }

                    let tvar = self.clone();
                    let arctvar = Arc::new(tvar);
                    rs.add(arctvar);

                    self.get_data()
                } else {
                    let written = scratch.unwrap();
                    let v: T = utils::version_to_dest(written);
                    v
                }
            }
            TransactionState::MarkedRollback => {
                // info!("Starting rolling back: {}", txn.get_id());
                txn.rolling_back();
                txn.on_abort::<T>();
                self.get_data()
            }
            TransactionState::RollingBack => {
                // Give some time to recover and prevent inconsistency with giving only the pure
                // data back.
                // std::thread::sleep(Duration::from_millis(10));
                self.get_data()
            }
            TransactionState::Suspended => {
                std::thread::sleep(Duration::from_millis(100));
                self.get_data()
            }
            TransactionState::RolledBack => {
                txn.rolled_back();
                panic!("Transaction rollback finalized.");
            }
            s => {
                panic!("Unexpected transaction state: {:?}", s);
            }
        }
    }

    ///
    /// Convenience over deref mut writes
    pub(crate) fn open_write_deref_mut(&mut self) -> T {
        self.open_write(self.get_data())
    }

    ///
    /// Explicit writes
    pub(crate) fn open_write(&mut self, data: T) -> T {
        // dbg!("OPEN WRITE");
        let txn = Txn::get_local();
        let state: &TransactionState = &*txn.state.get();

        match state {
            TransactionState::Committed | TransactionState::Unknown => self.get_data(),
            TransactionState::Active => {
                let mut ws = WriteSet::local();

                let this = Arc::new(self.clone());
                if ws.get_by_stamp::<T>(this.stamp).is_none() {
                    if self.is_locked() {
                        // TODO: throw abort
                        // panic!("WRITE: You can't lock and still continue processing");
                        txn.rollback();
                    }
                    self.modrev = self.modrev.saturating_add(1);
                    let this = Arc::new(self.clone());
                    ws.put::<T>(this, Arc::new(data.clone()));

                    // match txn.iso {
                    //     TransactionIsolation::ReadCommitted => {
                    //         dbg!("READ_COMMITTED_COMING");
                    //         if let Some(mut l) = GLOBAL_DELTAS.try_lock() {
                    //             let this = Arc::new(self.clone());
                    //             l.push(Version::Write(this));
                    //         }
                    //     },
                    //     _ => {
                    //         // todo!()
                    //     }
                    // }

                    self.data = Arc::new(data.clone());
                }
                self.data = Arc::new(data);
                self.get_data()
            }
            TransactionState::MarkedRollback
            | TransactionState::RollingBack
            | TransactionState::RolledBack => {
                // TODO: Normally aborted, I am still unsure that should I represent this as
                // full committed read or panic with a fault.
                // According to science serializable systems get panicked here.

                // panic!("Panic abort, no writes are possible.");
                txn.state.replace_with(|_| TransactionState::Unknown);
                self.get_data()
            }
            TransactionState::Suspended => {
                std::thread::sleep(Duration::from_millis(100));
                self.get_data()
            }
            s => {
                panic!("Unexpected transaction state: {:?}", s);
            }
        }
    }

    pub(crate) fn validate(&self) -> bool {
        let txn = Txn::get_local();
        let state: &TransactionState = &*txn.state.get();

        match state {
            TransactionState::Committed | TransactionState::Unknown => true,
            TransactionState::Active => {
                let free = self.is_not_locked_and_current();
                let pure = self.stamp <= TxnManager::rts();

                free & pure
            }
            TransactionState::MarkedRollback
            | TransactionState::RollingBack
            | TransactionState::RolledBack => false,
            s => {
                panic!("Unexpected transaction state: {:?}", s);
            }
        }
    }

    pub(crate) fn is_locked(&self) -> bool {
        self.lock.try_lock().is_none()
    }

    pub(crate) fn is_not_locked_and_current(&self) -> bool {
        !self.is_locked()
    }

    pub(crate) fn is_writer_held_by_current_thread(&self) -> bool {
        self.is_locked()
    }
}

impl<T: Any + Clone + Send + Sync> Deref for TVar<T> {
    type Target = T;

    fn deref(&self) -> &T {
        let x: *mut T = BoxMemory.allocate(self.open_read());
        unsafe { &*(x) }
    }
}

impl<T: 'static + Any + Clone + Send + Sync> DerefMut for TVar<T> {
    fn deref_mut(&mut self) -> &mut T {
        let x: *mut T = BoxMemory.allocate(self.open_write_deref_mut());
        unsafe { &mut *(x) }
    }
}

/// A type that can allocate and deallocate far heap memory.
pub(crate) trait Memory {
    /// Allocates memory.
    fn allocate<T>(&self, value: T) -> *mut T;

    /// Deallocates the memory associated with the supplied pointer.
    unsafe fn deallocate<T>(&self, pointer: *mut T);
}

#[derive(Copy, Clone, Debug)]
pub(crate) struct BoxMemory;

impl BoxMemory {
    pub(crate) fn reclaim<T>(&self, pointer: *const T) -> T {
        assert!(!pointer.is_null());
        unsafe { std::ptr::read_volatile::<T>(pointer as *mut T) }
    }

    pub(crate) fn reclaim_mut<T>(&self, pointer: *mut T) -> T {
        assert!(!pointer.is_null());
        unsafe { std::ptr::read_volatile::<T>(pointer) }
    }

    pub(crate) fn volatile_read<T: Clone>(&self, pointer: *mut T) -> T {
        assert!(!pointer.is_null());
        unsafe { std::ptr::read_volatile::<T>(pointer) }
    }

    pub(crate) fn deallocate_raw<T>(&self, p: *mut T) {
        unsafe {
            std::ptr::drop_in_place(p);
            dealloc(p as *mut u8, Layout::new::<T>());
        }
    }

    pub(crate) fn replace_with<T: Clone, X>(&self, ptr: *mut T, mut thunk: X)
    where
        X: FnMut(T) -> T,
    {
        let read = unsafe { std::ptr::read_volatile::<T>(ptr as *const T) };
        let res = thunk(read);
        unsafe { std::ptr::write_volatile::<T>(ptr, res) };
    }
}

impl Memory for BoxMemory {
    fn allocate<T>(&self, value: T) -> *mut T {
        Box::into_raw(Box::new(value))
    }

    unsafe fn deallocate<T>(&self, pointer: *mut T) {
        assert!(!pointer.is_null());
        Box::from_raw(pointer);
    }
}
