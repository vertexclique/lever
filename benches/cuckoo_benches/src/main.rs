use bustle::*;
use std::collections::HashMap;
use std::sync::RwLock;
use lever::prelude::*;

use std::hash::Hash;
// RwLock Table

#[derive(Clone)]
struct RwLockTable<K>(std::sync::Arc<RwLock<HashMap<K, u64>>>);

impl<K> Collection for RwLockTable<K>
    where
        K: Send + Sync + From<u64> + Copy + 'static + std::hash::Hash + Eq + std::fmt::Debug,
{
    type Handle = Self;
    fn with_capacity(capacity: usize) -> Self {
        Self(std::sync::Arc::new(RwLock::new(HashMap::with_capacity(
            capacity,
        ))))
    }

    fn pin(&self) -> Self::Handle {
        self.clone()
    }
}

impl<K> CollectionHandle for RwLockTable<K>
    where
        K: Send + Sync + From<u64> + Copy + 'static + std::hash::Hash + Eq + std::fmt::Debug,
{
    type Key = K;

    fn get(&mut self, key: &Self::Key) -> bool {
        self.0.read().unwrap().get(key).is_some()
    }

    fn insert(&mut self, key: &Self::Key) -> bool {
        self.0.write().unwrap().insert(*key, 1).is_none()
    }

    fn remove(&mut self, key: &Self::Key) -> bool {
        self.0.write().unwrap().remove(key).is_some()
    }

    fn update(&mut self, key: &Self::Key) -> bool {
        use std::collections::hash_map::Entry;
        let mut map = self.0.write().unwrap();
        if let Entry::Occupied(mut e) = map.entry(*key) {
            e.insert(1);
            true
        } else {
            false
        }
    }
}

// LOTable
#[derive(Clone)]
struct LOBenchTable<K>(std::sync::Arc<LOTable<K, u64>>)
where K: 'static + Send + Sync + Clone + Hash + Eq + Ord;

impl<K> Collection for LOBenchTable<K>
    where
        K: Send + Sync + From<u64> + Copy + 'static + std::hash::Hash + Eq + std::fmt::Debug + Ord,
{
    type Handle = Self;

    fn with_capacity(capacity: usize) -> Self {
        Self(std::sync::Arc::new(LOTable::with_capacity(capacity)))
    }

    fn pin(&self) -> Self::Handle {
        self.clone()
    }
}

impl<K> CollectionHandle for LOBenchTable<K>
    where
        K: Send + Sync + From<u64> + Copy + 'static + std::hash::Hash + Eq + std::fmt::Debug + Ord,
{
    type Key = K;

    fn get(&mut self, key: &Self::Key) -> bool {
        self.0.get(key).is_some()
    }

    fn insert(&mut self, key: &Self::Key) -> bool {
        self.0.insert(*key, 1).map(|x| x.is_none()).unwrap()
    }

    fn remove(&mut self, key: &Self::Key) -> bool {
        self.0.remove(key).map(|x| x.is_some()).unwrap()
    }

    fn update(&mut self, key: &Self::Key) -> bool {
        if let Some(_x) = self.0.get(key) {
            self.0.insert(*key, 1);
            true
        } else {
            false
        }
    }
}

fn main() {
    tracing_subscriber::fmt::init();
    // for n in 1..=num_cpus::get() {
    //     Workload::new(n, Mix::read_heavy()).run::<RwLockTable<u64>>();
    // }

    println!("========================");

    // for n in 1..=num_cpus::get() {
        Workload::new(3, Mix::read_heavy()).run::<LOBenchTable<u64>>();
    // }
}