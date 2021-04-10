use crate::sync::atomics::AtomicBox;
use crate::txn::prelude::*;

use std::collections::hash_map::{Iter, Keys, RandomState};
use std::collections::HashMap;

use anyhow::*;
use std::collections::hash_map;
use std::fmt;
use std::hash::Hash;
use std::hash::{BuildHasher, Hasher};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

const DEFAULT_CAP: usize = 1024;

pub struct LOTableBuilder {
    cc: TransactionConcurrency,
    iso: TransactionIsolation,
    timeout: usize,
    size: usize,
    label: String,
    cap: usize,
    hasher: RandomState,
}

impl LOTableBuilder {
    pub fn new() -> Self {
        Self {
            cc: TransactionConcurrency::Optimistic,
            iso: TransactionIsolation::RepeatableRead,
            timeout: 100_usize,
            size: 1_usize,
            label: "default".into(),
            cap: DEFAULT_CAP,
            hasher: RandomState::new(),
        }
    }

    pub fn with_concurrency(self, cc: TransactionConcurrency) -> Self {
        Self { cc, ..self }
    }

    pub fn with_isolation(self, iso: TransactionIsolation) -> Self {
        Self { iso, ..self }
    }

    pub fn with_timeout(self, timeout: usize) -> Self {
        Self { timeout, ..self }
    }
    pub fn with_size(self, size: usize) -> Self {
        Self { size, ..self }
    }

    pub fn with_label(self, label: String) -> Self {
        Self { label, ..self }
    }

    pub fn with_capacity(self, cap: usize) -> Self {
        Self { cap, ..self }
    }

    pub fn with_hasher(self, hasher: RandomState) -> Self {
        Self { hasher, ..self }
    }

    pub fn build<K, V>(self) -> LOTable<K, V>
    where
        K: PartialEq + Eq + Hash + Clone + Send + Sync,
        V: Clone + Send + Sync,
    {
        let txn_man = Arc::new(TxnManager {
            txid: Arc::new(AtomicU64::new(GLOBAL_VCLOCK.load(Ordering::SeqCst))),
        });

        let txn: Arc<Txn> =
            Arc::new(txn_man.txn_build(self.cc, self.iso, self.timeout, self.size, self.label));

        LOTable::with_cap_hash_and_txn(self.cap, self.hasher, txn)
    }
}

#[derive(Clone)]
///
/// Lever Transactional Table implementation with [Optimistic](TransactionConcurrency::Optimistic)
/// concurrency and [RepeatableRead](TransactionIsolation::RepeatableRead) isolation.
///
/// Transactional hash table fully concurrent and as long as no conflicts are made
/// it is both lock and wait free.
pub struct LOTable<K, V, S = RandomState>
where
    K: 'static + PartialEq + Eq + Hash + Clone + Send + Sync,
    V: 'static + Clone + Send + Sync,
    S: BuildHasher,
{
    latch: Vec<TVar<Arc<AtomicBox<Container<K, V>>>>>,
    txn_man: Arc<TxnManager>,
    txn: Arc<Txn>,
    hash_builder: S,
}

impl<K, V> LOTable<K, V, RandomState>
where
    K: PartialEq + Eq + Hash + Clone + Send + Sync,
    V: Clone + Send + Sync,
{
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CAP)
    }

    pub fn with_capacity(cap: usize) -> Self {
        Self::with_capacity_and_hasher(cap, RandomState::new())
    }
}

impl<K, V, S> LOTable<K, V, S>
where
    K: PartialEq + Eq + Hash + Clone + Send + Sync,
    V: Clone + Send + Sync,
    S: BuildHasher,
{
    fn with_cap_hash_and_txn(cap: usize, hasher: S, txn: Arc<Txn>) -> LOTable<K, V, S> {
        let txn_man = Arc::new(TxnManager {
            txid: Arc::new(AtomicU64::new(GLOBAL_VCLOCK.load(Ordering::SeqCst))),
        });

        Self {
            latch: vec![TVar::new(Arc::new(AtomicBox::new(Container(HashMap::default())))); cap],
            txn_man,
            txn,
            hash_builder: hasher,
        }
    }
    fn with_capacity_and_hasher(cap: usize, hasher: S) -> LOTable<K, V, S> {
        let txn_man = Arc::new(TxnManager {
            txid: Arc::new(AtomicU64::new(GLOBAL_VCLOCK.load(Ordering::SeqCst))),
        });

        let txn: Arc<Txn> = Arc::new(txn_man.txn_build(
            TransactionConcurrency::Optimistic,
            TransactionIsolation::RepeatableRead,
            100_usize,
            1_usize,
            "default".into(),
        ));

        Self {
            latch: vec![TVar::new(Arc::new(AtomicBox::new(Container(HashMap::default())))); cap],
            txn_man,
            txn,
            hash_builder: hasher,
        }
    }

    #[inline]
    pub fn insert(&self, k: K, v: V) -> Result<Arc<Option<V>>> {
        let tvar = self.seek_tvar(&k);

        let container = self.txn.begin(|t| t.read(&tvar))?;

        let previous: Arc<AtomicBox<Option<V>>> = Arc::new(AtomicBox::new(None));
        container.replace_with(|r| {
            let mut entries = r.0.clone();
            let p = entries.insert(k.clone(), v.clone());
            previous.replace_with(|_| p.clone());
            Container(entries)
        });

        previous.extract()
    }

    #[inline]
    pub fn remove(&self, k: &K) -> Result<Arc<Option<V>>> {
        let tvar = self.seek_tvar(&k);

        let container = self.txn.begin(|t| t.read(&tvar))?;

        let previous: Arc<AtomicBox<Option<V>>> = Arc::new(AtomicBox::new(None));
        container.replace_with(|r| {
            let mut c = r.0.clone();
            let p = c.remove(k);
            previous.replace_with(|_| p.clone());
            Container(c)
        });

        previous.extract()
    }

    #[inline]
    pub fn get(&self, k: &K) -> Option<V> {
        let tvar = self.seek_tvar(k);

        self.txn
            .begin(|t| {
                let container = t.read(&tvar);
                let entries = container.get();
                entries.0.get(k).cloned()
            })
            .unwrap_or(None)
    }

    #[inline]
    pub fn replace_with<F>(&self, k: &K, f: F) -> Option<V>
    where
        F: Fn(Option<&V>) -> Option<V>,
    {
        let tvar = self.seek_tvar(k);

        self.txn
            .begin(|t| {
                let container = t.read(&tvar);
                let entries = container.get();
                f(entries.0.get(k))
            })
            .unwrap_or(None)
    }

    #[inline]
    pub fn replace_with_mut<F>(&self, k: &K, mut f: F) -> Option<V>
    where
        F: FnMut(&mut Option<V>) -> &mut Option<V>,
    {
        let tvar = self.seek_tvar(k);

        self.txn
            .begin(|t| {
                let container = t.read(&tvar);
                let entries = container.get();
                let mut mv = entries.0.get(k).cloned();
                f(&mut mv).clone()
            })
            .unwrap_or(None)
    }

    #[inline]
    pub fn contains_key(&self, k: &K) -> bool {
        let tvar = self.seek_tvar(&k);

        self.txn
            .begin(|t| {
                let container = t.read(&tvar);
                container.get().0.contains_key(k)
            })
            .unwrap_or(false)
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.latch
            .first()
            .map(move |b| {
                self.txn
                    .begin(|t| {
                        let container = t.read(&b);
                        container.get().0.len()
                    })
                    .unwrap_or(0_usize)
            })
            .unwrap_or(0_usize)
    }

    #[inline]
    pub fn iter(&self) -> LOIter<K, V> {
        LOIter {
            idx: 0,
            inner: None,
            reader: HashMap::default(),
            current_frame: 0,
            latch_snapshot: self.latch.clone(),
            txn: self.txn.clone(),
        }
    }

    #[inline]
    pub fn clear(&self) {
        self.latch.iter().for_each(move |b| {
            let _ = self.txn.begin(|t| {
                let container = t.read(&b);
                container.replace_with(|_r| Container(HashMap::default()));
            });
        });
        // TODO: (vcq): Shrink to fit as a optimized table.
        // self.latch.shrink_to_fit();
    }

    pub fn keys<'table>(&'table self) -> impl Iterator<Item = K> + 'table {
        let buckets: Vec<K> = self
            .latch
            .first()
            .iter()
            .flat_map(move |b| {
                self.txn
                    .begin(|t| {
                        let container = t.read(&b);
                        container
                            .get()
                            .0
                            .keys()
                            .into_iter()
                            .map(Clone::clone)
                            .collect::<Vec<K>>()
                    })
                    .unwrap_or(vec![])
            })
            .collect();

        buckets.into_iter()
    }

    pub fn values<'table>(&'table self) -> impl Iterator<Item = V> + 'table {
        let buckets: Vec<V> = self
            .latch
            .first()
            .iter()
            .flat_map(move |b| {
                self.txn
                    .begin(|t| {
                        let container = t.read(&b);
                        container
                            .get()
                            .0
                            .values()
                            .into_iter()
                            .map(Clone::clone)
                            .collect::<Vec<V>>()
                    })
                    .unwrap_or(vec![])
            })
            .collect();

        buckets.into_iter()
    }

    fn hash(&self, key: &K) -> usize {
        let mut hasher = self.hash_builder.build_hasher();
        key.hash(&mut hasher);
        hasher.finish() as usize % self.latch.len()
    }

    fn seek_tvar(&self, key: &K) -> TVar<Arc<AtomicBox<Container<K, V>>>> {
        self.latch[self.hash(key)].clone()
    }

    fn fetch_frame(&self, frame_id: usize) -> hash_map::HashMap<K, V> {
        let frame_tvar = self.latch[frame_id].clone();
        match self.txn.begin(|t| t.read(&frame_tvar)) {
            Ok(init_frame) => init_frame.get().0.clone(),
            Err(_) => HashMap::new(),
        }
    }

    ////////////////////////////////////////////////////////////////////////////////
    ////////// Transactional Area
    ////////////////////////////////////////////////////////////////////////////////

    pub fn tx_manager(&self) -> Arc<TxnManager> {
        self.txn_man.clone()
    }
}

#[derive(Clone)]
struct Container<K, V>(HashMap<K, V>)
where
    K: PartialEq + Hash + Clone + Send + Sync,
    V: Clone + Send + Sync;

impl<K, V, S> Default for LOTable<K, V, S>
where
    K: 'static + PartialEq + Eq + Hash + Clone + Send + Sync,
    V: 'static + Clone + Send + Sync,
    S: Default + BuildHasher,
{
    /// Creates an empty `LOTable<K, V, S>`, with the `Default` value for the hasher.
    #[inline]
    fn default() -> LOTable<K, V, S> {
        LOTable::with_capacity_and_hasher(128, Default::default())
    }
}

impl<K, V, S> fmt::Debug for LOTable<K, V, S>
where
    K: 'static + PartialEq + Eq + Hash + Clone + Send + Sync + fmt::Debug,
    V: 'static + Clone + Send + Sync + fmt::Debug,
    S: std::hash::BuildHasher,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

pub struct LOIter<'it, K, V>
where
    K: 'static + PartialEq + Eq + Hash + Clone + Send + Sync,
    V: 'static + Clone + Send + Sync,
{
    idx: usize,
    inner: Option<hash_map::Iter<'it, K, V>>,
    reader: HashMap<K, V>,
    current_frame: usize,
    latch_snapshot: Vec<TVar<Arc<AtomicBox<Container<K, V>>>>>,
    txn: Arc<Txn>,
}

impl<'it, K, V> Iterator for LOIter<'it, K, V>
where
    K: 'static + PartialEq + Eq + Hash + Clone + Send + Sync,
    V: 'static + Clone + Send + Sync,
{
    type Item = (K, V);

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        if self.idx == 0 {
            let tvar = &self.latch_snapshot[self.current_frame];
            if let Ok(read) = self.txn.begin(|t| {
                let frame = t.read(&tvar);
                frame.get().0.clone()
            }) {
                self.reader = read;
                self.inner = Some(unsafe { std::mem::transmute(self.reader.iter()) });
            }
        }

        let read_iter = self.inner.as_mut().unwrap();
        if let Some(x) = read_iter.next() {
            self.idx += 1;
            self.inner = Some(read_iter.clone());
            Some((x.0.clone(), x.1.clone()))
        } else {
            if self.idx == self.reader.len() {
                self.current_frame += 1;
                self.idx = 0;
            }
            None
        }
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let tvar = &self.latch_snapshot[self.current_frame];
        if let Ok(frame_len) = self.txn.begin(|t| t.read(&tvar)) {
            // TODO: (frame_len, Some(max_bound)) is possible.
            // Written like this to not overshoot the alloc
            (frame_len.get().0.len(), None)
        } else {
            (0, None)
        }
    }
}

#[cfg(test)]
mod lotable_tests {
    use super::LOTable;

    #[test]
    fn iter_generator() {
        let lotable: LOTable<String, u64> = LOTable::new();
        lotable.insert("Saudade0".to_string(), 123123);
        lotable.insert("Saudade0".to_string(), 123);
        lotable.insert("Saudade1".to_string(), 123123);
        lotable.insert("Saudade2".to_string(), 123123);
        lotable.insert("Saudade3".to_string(), 123123);
        lotable.insert("Saudade4".to_string(), 123123);
        lotable.insert("Saudade5".to_string(), 123123);

        lotable.insert("123123".to_string(), 123123);
        lotable.insert("1231231".to_string(), 123123);
        lotable.insert("1231232".to_string(), 123123);
        lotable.insert("1231233".to_string(), 123123);
        lotable.insert("1231234".to_string(), 123123);
        lotable.insert("1231235".to_string(), 123123);

        let res: Vec<(String, u64)> = lotable.iter().collect();
        assert_eq!(res.len(), 12);

        assert_eq!(lotable.get(&"Saudade0".to_string()), Some(123));
    }

    #[test]
    fn values_iter_generator() {
        let lotable: LOTable<String, u64> = LOTable::new();

        (0..100).into_iter().for_each(|_i| {
            lotable.insert("Saudade0".to_string(), 123123);
            lotable.insert("Saudade0".to_string(), 123);
            lotable.insert("Saudade1".to_string(), 123123);
            lotable.insert("Saudade2".to_string(), 123123);
            lotable.insert("Saudade3".to_string(), 123123);
            lotable.insert("Saudade4".to_string(), 123123);
            lotable.insert("Saudade5".to_string(), 123123);

            lotable.insert("123123".to_string(), 123123);
            lotable.insert("1231231".to_string(), 123123);
            lotable.insert("1231232".to_string(), 123123);
            lotable.insert("1231233".to_string(), 123123);
            lotable.insert("1231234".to_string(), 123123);
            lotable.insert("1231235".to_string(), 123123);

            let res: Vec<u64> = lotable.values().into_iter().collect();
            // dbg!(&res);
            assert_eq!(res.len(), 12);
        });

        lotable.clear();
        let res: Vec<u64> = lotable.values().into_iter().collect();
        assert_eq!(res.len(), 0);

        (0..1_000).into_iter().for_each(|i| {
            lotable.insert(format!("{}", i), i as u64);

            let resvals: Vec<u64> = lotable.values().into_iter().collect();
            // dbg!(&resvals);
            assert_eq!(resvals.len(), i + 1);
        });

        lotable.clear();
        let res: Vec<u64> = lotable.values().into_iter().collect();
        assert_eq!(res.len(), 0);

        (0..1_000).into_iter().for_each(|i| {
            lotable.insert(format!("{}", i), i as u64);

            let reskeys: Vec<String> = lotable.keys().into_iter().collect();
            // dbg!(&reskeys);
            assert_eq!(reskeys.len(), i + 1);
        });
    }
}
