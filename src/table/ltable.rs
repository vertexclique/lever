use crate::txn::prelude::*;
use std::{
    cell::UnsafeCell,
    collections::{
        hash_map::{RandomState, Values},
        BTreeMap, HashMap,
    },
    hash::{self, BuildHasher, Hash},
    sync::atomic::AtomicPtr,
};

use std::{
    borrow::Borrow,
    fmt,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

#[derive(Clone)]
pub struct LTable<K, V>
where
    K: PartialEq + Hash,
    V: Clone,
{
    name: String,
    latch: HashMap<K, V>,
    txn_man: Arc<TxnManager>,
}

impl<K, V> LTable<K, V>
where
    K: PartialEq + Hash + Eq,
    V: Clone,
{
    pub fn create(name: String) -> Self {
        // TODO: Separate data from the latch access.

        let txn_man = Arc::new(TxnManager {
            txid: Arc::new(AtomicU64::new(GLOBAL_VCLOCK.load(Ordering::SeqCst))),
        });

        Self {
            latch: HashMap::with_capacity(100),
            name,
            txn_man,
        }
    }

    #[inline]
    pub fn insert(&mut self, k: K, v: V) -> Option<V> {
        self.latch.insert(k, v)
    }

    #[inline]
    pub fn get<Q: ?Sized>(&self, k: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.latch.get(k)
    }

    pub fn values(&self) -> Values<K, V> {
        self.latch.values()
    }

    #[inline]
    pub fn clear(&mut self) {
        self.latch.clear();
        // TODO: Shrink to fit as a optimized table.
        // self.latch.shrink_to_fit();
    }

    pub fn transactions(&self) -> Arc<TxnManager> {
        self.txn_man.clone()
    }
}

unsafe impl<K, V> Send for LTable<K, V>
where
    K: PartialEq + Clone + hash::Hash + Send,
    V: Send + Clone,
{
}
unsafe impl<K, V> Sync for LTable<K, V>
where
    K: PartialEq + Clone + hash::Hash + Sync,
    V: Sync + Clone,
{
}

impl<K, V> fmt::Debug for LTable<K, V>
where
    K: PartialEq + Hash + fmt::Debug,
    V: Clone + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LTable")
            .field("table", &self.latch)
            .finish()
    }
}

#[cfg(test)]
mod ltable_tests {
    use super::*;

    #[test]
    fn ltable_creation() {
        let _ltable = LTable::<String, String>::create("test1".to_owned());
    }

    #[test]
    #[allow(unused_assignments)]
    fn ltable_transaction_begin() {
        let ltable = LTable::<String, String>::create("test1".to_owned());
        let txn = ltable.transactions().txn_build(
            TransactionConcurrency::Optimistic,
            TransactionIsolation::RepeatableRead,
            100_usize,
            1_usize,
            "txn_label".into(),
        );

        let mut tvar = TVar::new(ltable);

        let mut res = txn
            .begin(|t: &mut Txn| {
                let mut x = t.read(&tvar);
                x.insert("taetigkeit".into(), "ingenieur".into());
                // dbg!(&tvar.get_data());
                t.write(&mut tvar, x.clone());
                t.read(&tvar)
            })
            .unwrap();

        dbg!(&res);

        assert_eq!(
            *res.get("taetigkeit".into()).unwrap(),
            "ingenieur".to_string()
        );

        txn.begin(|t| {
            let x = t.read(&tvar);
            dbg!(&x);
            assert_eq!(x.get("taetigkeit".into()), Some(&String::from("ingenieur")));
        })
        .unwrap();

        res = txn
            .begin(|t| {
                let mut x = t.read(&tvar);
                x.insert("a".into(), "b".into());
                x
            })
            .unwrap();

        // Repetitive insert 2
        res = txn
            .begin(|te| {
                let mut x = te.read(&tvar);
                x.insert("a".into(), "b".into());
                x
            })
            .unwrap();

        // Repeatable Reads
        assert_eq!(
            res.get("taetigkeit".into()),
            Some(&String::from("ingenieur"))
        );

        // Committed Reads - Read Committed
        assert_eq!(res.get("a".into()).cloned().unwrap(), "b".to_string());
    }

    #[test]
    #[allow(unused_assignments)]
    fn ltable_transaction_tvar() {
        let ltable = LTable::<String, String>::create("test1".to_owned());
        let txn = ltable.transactions().txn_build(
            TransactionConcurrency::Optimistic,
            TransactionIsolation::RepeatableRead,
            100_usize,
            1_usize,
            "txn_label".into(),
        );

        let mut tvar = TVar::new(ltable);

        let mut res = txn
            .begin(|t: &mut Txn| {
                let mut x = t.read(&tvar);
                x.insert("taetigkeit".into(), "ingenieur".into());
                // dbg!(&tvar.get_data());
                t.write(&mut tvar, x.clone());
                t.read(&tvar)
            })
            .unwrap();

        dbg!("TVAR_RES", &res);
        tvar.open_write(res.clone());

        dbg!("TVAR", tvar.get_data());

        assert_eq!(
            *res.get("taetigkeit".into()).unwrap(),
            "ingenieur".to_string()
        );

        txn.begin(|t| {
            let x = t.read(&tvar);
            dbg!(&x);
            assert_eq!(x.get("taetigkeit".into()), Some(&String::from("ingenieur")));
        })
        .unwrap();

        res = txn
            .begin(|t| {
                let mut x = t.read(&tvar);
                x.insert("a".into(), "b".into());
                x
            })
            .unwrap();

        // Repetitive insert 2
        res = txn
            .begin(|te| {
                let mut x = te.read(&tvar);
                x.insert("a".into(), "b".into());
                x
            })
            .unwrap();

        // Repeatable Reads
        assert_eq!(
            res.get("taetigkeit".into()),
            Some(&String::from("ingenieur"))
        );

        // Committed Reads - Read Committed
        assert_eq!(res.get("a".into()).cloned().unwrap(), "b".to_string());
    }

    fn sum_table(table: &LTable<String, i64>) -> i64 {
        table.values().map(|f| *f).sum::<i64>()
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn ltable_transaction_threaded_light() {
        let mut ltable1 = LTable::<String, i64>::create("alice_1_banking".to_owned());
        let mut ltable2 = LTable::<String, i64>::create("alice_2_banking".to_owned());
        let ltable3 = LTable::<String, i64>::create("bob_banking".to_owned());

        ltable1.insert("alice1_init".into(), 50);
        ltable2.insert("alice2_init".into(), 50);

        let txn = TxnManager::manager().txn_build(
            TransactionConcurrency::Optimistic,
            TransactionIsolation::Serializable,
            100_usize,
            1_usize,
            "txn_label".into(),
        );

        let alice_accounts = [TVar::new(ltable1), TVar::new(ltable2)];
        let bob_account = TVar::new(ltable3);

        for _ in 0..10 {
            let txn = txn.clone();

            let mut threads = vec![];

            for thread_no in 0..2 {
                let txn = txn.clone();
                let mut alice_accounts = alice_accounts.clone();
                let mut bob_account = bob_account.clone();

                let t = std::thread::Builder::new()
                    .name(format!("t_{}", thread_no))
                    .spawn(move || {
                        // assert!(!is_contended());

                        for i in 0..2 {
                            if (i + thread_no) % 2 == 0 {
                                // try to transfer
                                let withdrawal_account = thread_no % alice_accounts.len();

                                txn.begin(|t| {
                                    let mut a0 = t.read(&alice_accounts[0]);
                                    let mut a1 = t.read(&alice_accounts[1]);
                                    let mut b = t.read(&bob_account);

                                    let sum = sum_table(&a0) + sum_table(&a1);

                                    if sum >= 100 {
                                        if withdrawal_account == 0 {
                                            a0.insert(format!("from_t_{}", thread_no), -100);
                                        } else {
                                            a1.insert(format!("from_t_{}", thread_no), -100);
                                        }
                                        b.insert(format!("to_t_{}", thread_no), 100);
                                    }

                                    t.write(&mut alice_accounts[0], a0.clone());
                                    t.write(&mut alice_accounts[1], a1.clone());
                                    t.write(&mut bob_account, b.clone());
                                })
                                .unwrap();
                            } else {
                                // assert that the sum of alice's accounts
                                // never go negative
                                // let r0: &LTable<String, i64> = &*alice_accounts[0];
                                let r = txn
                                    .begin(|_t| {
                                        (
                                            sum_table(&*alice_accounts[0]),
                                            sum_table(&*alice_accounts[1]),
                                            sum_table(&*bob_account),
                                        )
                                    })
                                    .unwrap();

                                // dbg!("TESTRESULT", &r);

                                assert!(
                                    r.0 + r.1 >= 0,
                                    "possible write skew anomaly detected! expected the \
                                         sum of alice's accounts to be >= 0. observed values: {:?}",
                                    r
                                );

                                assert_ne!(
                                    r.2, 200,
                                    "A double-transfer to bob was detected! \
                                     read values: {:?}",
                                    r
                                );

                                // reset accounts
                                txn.begin(|_t| {
                                    (*alice_accounts[0]).clear();
                                    (*alice_accounts[0]).insert("alice1_init".into(), 50);
                                    (*alice_accounts[1]).clear();
                                    (*alice_accounts[1]).insert("alice2_init".into(), 50);
                                    (*bob_account).clear();
                                })
                                .unwrap();
                            }
                        }
                    })
                    .unwrap();

                threads.push(t);
            }

            for t in threads.into_iter() {
                t.join().unwrap();
            }
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn ltable_transaction_threaded_heavy() {
        let mut ltable1 = LTable::<String, i64>::create("alice_1_banking".to_owned());
        let mut ltable2 = LTable::<String, i64>::create("alice_2_banking".to_owned());
        let ltable3 = LTable::<String, i64>::create("bob_banking".to_owned());

        ltable1.insert("alice1_init".into(), 50);
        ltable2.insert("alice2_init".into(), 50);

        let txn = TxnManager::manager().txn_build(
            TransactionConcurrency::Optimistic,
            TransactionIsolation::Serializable,
            100_usize,
            1_usize,
            "txn_label".into(),
        );

        let alice_accounts = [TVar::new(ltable1), TVar::new(ltable2)];
        let bob_account = TVar::new(ltable3);

        for _ in 0..10 {
            let txn = txn.clone();

            let mut threads = vec![];

            for thread_no in 0..20 {
                let txn = txn.clone();
                let mut alice_accounts = alice_accounts.clone();
                let mut bob_account = bob_account.clone();

                let t = std::thread::Builder::new()
                    .name(format!("t_{}", thread_no))
                    .spawn(move || {
                        // assert!(!is_contended());

                        for i in 0..500 {
                            if (i + thread_no) % 2 == 0 {
                                // try to transfer
                                let withdrawal_account = thread_no % alice_accounts.len();

                                let _ = txn.begin(|t| {
                                    let mut a0 = t.read(&alice_accounts[0]);
                                    let mut a1 = t.read(&alice_accounts[1]);
                                    let mut b = t.read(&bob_account);

                                    let sum = sum_table(&a0) + sum_table(&a1);

                                    if sum >= 100 {
                                        if withdrawal_account == 0 {
                                            a0.insert(format!("from_t_{}", thread_no), -100);
                                        } else {
                                            a1.insert(format!("from_t_{}", thread_no), -100);
                                        }
                                        b.insert(format!("to_t_{}", thread_no), 100);
                                    }

                                    t.write(&mut alice_accounts[0], a0.clone());
                                    t.write(&mut alice_accounts[1], a1.clone());
                                    t.write(&mut bob_account, b.clone());
                                });
                            } else {
                                // assert that the sum of alice's accounts
                                // never go negative
                                let r = txn
                                    .begin(|_t| {
                                        (
                                            sum_table(&*alice_accounts[0]),
                                            sum_table(&*alice_accounts[1]),
                                            sum_table(&*bob_account),
                                        )
                                    })
                                    .unwrap();

                                // dbg!("TESTRESULT", &r);

                                assert!(
                                    r.0 + r.1 >= 0,
                                    "possible write skew anomaly detected! expected the \
                                         sum of alice's accounts to be >= 0. observed values: {:?}",
                                    r
                                );

                                assert_ne!(
                                    r.2, 200,
                                    "A double-transfer to bob was detected! \
                                     read values: {:?}",
                                    r
                                );

                                // reset accounts
                                let _ = txn.begin(|_t| {
                                    (*alice_accounts[0]).clear();
                                    (*alice_accounts[0]).insert("alice1_init".into(), 50);
                                    (*alice_accounts[1]).clear();
                                    (*alice_accounts[1]).insert("alice2_init".into(), 50);
                                    (*bob_account).clear();
                                });
                            }
                        }
                    })
                    .unwrap();

                threads.push(t);
            }

            for t in threads.into_iter() {
                t.join().unwrap();
            }
        }
    }
}
