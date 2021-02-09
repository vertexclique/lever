use crate::sync::atomics::AtomicBox;
use anyhow::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use std::hash::Hash;
use std::hash::{BuildHasher, Hasher};
use std::{
    alloc::Layout,
    collections::hash_map::{Iter, Keys, RandomState},
};

const HOP_RANGE: usize = 1 << 5;
const ADD_RANGE: usize = 1 << 8;
const MAX_SEGMENTS: usize = 1 << 20;
const HOLE_EXIST: isize = -1;
const ALREADY_FILLED: isize = -2;

// TODO: KeyState should enable overflows on binary heap or such.
enum KeyState {
    Index(isize),
    HoleExist,
    AlreadyFilled,
}

///
/// Lever Neighborhood based cache-oblivious concurrent table.
///
/// Designed for fast access under heavy contention.
/// Best for related lookups in the known key space.
/// Also best for buffer management.
pub struct HOPTable<K, V, S = RandomState>
where
    K: 'static + PartialEq + Eq + Hash + Clone + Send + Sync,
    V: 'static + Clone + Send + Sync,
    S: BuildHasher,
{
    segments: Vec<Bucket<K, V>>,
    max_segments: usize,
    hash_builder: S,
}

impl<K, V> HOPTable<K, V, RandomState>
where
    K: PartialEq + Eq + Hash + Clone + Send + Sync,
    V: Clone + Send + Sync,
{
    pub fn new() -> Self {
        Self::with_capacity(MAX_SEGMENTS)
    }

    pub fn with_capacity(cap: usize) -> Self {
        assert!(
            (cap != 0) && ((cap & (cap - 1)) == 0),
            "Capacity should be power of 2"
        );
        Self::with_capacity_and_hasher(cap, RandomState::new())
    }
}

impl<K, V, S> HOPTable<K, V, S>
where
    K: PartialEq + Eq + Hash + Clone + Send + Sync,
    V: Clone + Send + Sync,
    S: BuildHasher,
{
    fn with_capacity_and_hasher(cap: usize, hasher: S) -> HOPTable<K, V, S> {
        Self {
            segments: (0..cap + (1 << 8)).map(|_| Bucket::default()).collect(),
            max_segments: cap,
            hash_builder: hasher,
        }
    }

    fn hash(&self, key: &K) -> usize {
        let mut hasher = self.hash_builder.build_hasher();
        key.hash(&mut hasher);
        hasher.finish() as usize & (self.max_segments - 1)
    }

    fn seek_segment(&self, key: &K) -> Option<Bucket<K, V>> {
        let idx = self.key_index(key);
        if idx != !0 {
            Some(self.segments[idx as usize].clone())
        } else {
            None
        }
    }

    // FIXME: no pub
    pub(crate) fn key_index(&self, k: &K) -> isize {
        let hash = self.hash(k);
        let start_bucket = self.segments[hash].clone();
        let mut mask = 1;

        for i in (0..HOP_RANGE).into_iter() {
            if (mask & start_bucket.hop_info.load(Ordering::Acquire)) >= 1 {
                let check_bucket = self.segments[hash + i].clone();
                let keyv = self.extract(check_bucket.key.get());
                if Some(k) == keyv.as_ref() {
                    return (hash + i) as isize;
                }
            }
            mask = mask << 1;
        }

        HOLE_EXIST
    }

    fn atomic_remove(&self, k: &K) -> Arc<Option<V>> {
        let hash = self.hash(k);
        let start_bucket = self.segments[hash].clone();

        let remove_bucket_idx = self.key_index(k);
        let distance = remove_bucket_idx as usize - hash;

        if remove_bucket_idx > !0 {
            let remove_bucket = self.segments[remove_bucket_idx as usize].clone();
            let rc = remove_bucket.data.get();
            remove_bucket.key.replace_with(|_| None);
            remove_bucket.data.replace_with(|_| None);

            let st = start_bucket.hop_info.load(Ordering::Acquire);
            start_bucket
                .hop_info
                .store(st & !(1 << distance), Ordering::Relaxed);
            return rc;
        }

        Arc::new(None)
    }

    #[inline]
    pub fn remove(&self, k: &K) -> Result<Arc<Option<V>>> {
        Ok(self.atomic_remove(k))
        // TODO: Fallback Lock
    }

    #[inline]
    pub fn get(&self, k: &K) -> Option<V> {
        if let Some(seg) = self.seek_segment(k) {
            let val = seg.data.get();
            return self.extract(val).clone();
        }

        None
    }

    fn extract<A>(&self, val: Arc<A>) -> &A {
        unsafe { &*Arc::downgrade(&val).as_ptr() }
    }

    fn atomic_insert(
        &self,
        start_bucket: &Bucket<K, V>,
        free_bucket: &Bucket<K, V>,
        k: &K,
        v: &V,
        free_distance: usize,
    ) -> bool {
        if self.key_index(k) == HOLE_EXIST {
            let sbhi = start_bucket.hop_info.load(Ordering::Acquire);
            start_bucket
                .hop_info
                .store(sbhi | (1 << free_distance), Ordering::Release);
            free_bucket.data.replace_with(|_| Some(v.clone()));
            free_bucket.key.replace_with(|_| Some(k.clone()));
            return true;
        }

        false
    }

    #[inline]
    pub fn insert(&self, k: K, v: V) -> Result<Arc<Option<V>>> {
        if let Some(_) = self.seek_segment(&k) {
            let _ = self.remove(&k);
        }

        self.new_insert(k, v)
    }

    fn new_insert(&self, k: K, v: V) -> Result<Arc<Option<V>>> {
        let mut val = 1;

        let hash = self.hash(&k);
        let start_bucket = self.segments[hash].clone();

        let mut free_bucket_idx = hash;
        let mut free_bucket = self.segments[free_bucket_idx].clone();
        let mut free_distance = 0;

        for _ in (free_distance..ADD_RANGE).into_iter() {
            if free_bucket.key.get().is_none() {
                break;
            }
            free_distance += 1;
            free_bucket_idx += 1;

            free_bucket = self.segments[free_bucket_idx].clone();
        }

        if free_distance < ADD_RANGE {
            while let true = 0 != val {
                if free_distance < HOP_RANGE {
                    if self.atomic_insert(&start_bucket, &free_bucket, &k, &v, free_distance) {
                        return Ok(Arc::new(Some(v)));
                    } else {
                        return Ok(Arc::new(None));
                    }
                } else {
                    let closest_binfo =
                        self.find_closer_bucket(free_bucket_idx, free_distance, val);
                    free_distance = closest_binfo[0];
                    val = closest_binfo[1];
                    free_bucket_idx = closest_binfo[2];
                    free_bucket = self.segments[free_bucket_idx].clone();
                }
            }
        }

        Ok(Arc::new(None))
    }

    fn find_closer_bucket(
        &self,
        free_bucket_index: usize,
        mut free_distance: usize,
        val: usize,
    ) -> [usize; 3] {
        let mut result = [0; 3];
        let mut move_bucket_index = free_bucket_index - (HOP_RANGE - 1);
        let mut move_bucket = self.segments[move_bucket_index].clone();
        for free_dist in (1..HOP_RANGE).rev() {
            let start_hop_info = move_bucket.hop_info.load(Ordering::Acquire);
            let mut move_free_distance: isize = !0;
            let mut mask = 1;
            for i in (0..free_dist).into_iter() {
                if (mask & start_hop_info) >= 1 {
                    move_free_distance = i as isize;
                    break;
                }
                mask = mask << 1;
            }

            if !0 != move_free_distance {
                if start_hop_info == move_bucket.hop_info.load(Ordering::Acquire) {
                    let new_free_bucket_index = move_bucket_index + move_free_distance as usize;
                    let new_free_bucket = self.segments[new_free_bucket_index].clone();
                    let mbhi = move_bucket.hop_info.load(Ordering::Acquire);
                    // Updates move bucket's hop data, to indicate the newly inserted bucket
                    move_bucket
                        .hop_info
                        .store(mbhi | (1 << free_dist), Ordering::SeqCst);
                    self.segments[free_bucket_index]
                        .data
                        .replace_with(|_ex| self.extract(new_free_bucket.data.get()).clone());
                    self.segments[free_bucket_index]
                        .key
                        .replace_with(|_ex| self.extract(new_free_bucket.key.get()).clone());

                    new_free_bucket.key.replace_with(|_| None);
                    new_free_bucket.data.replace_with(|_| None);

                    // Updates move bucket's hop data, to indicate the deleted bucket
                    move_bucket.hop_info.store(
                        move_bucket.hop_info.load(Ordering::SeqCst) & !(1 << move_free_distance),
                        Ordering::SeqCst,
                    );
                    free_distance = free_distance - free_dist + move_free_distance as usize;
                    result[0] = free_distance;
                    result[1] = val;
                    result[2] = new_free_bucket_index;
                    return result;
                }
            }
            move_bucket_index = move_bucket_index + 1;
            move_bucket = self.segments[move_bucket_index].clone();
        }

        self.segments[free_bucket_index].key.replace_with(|_| None);
        result[0] = 0;
        result[1] = 0;
        result[2] = 0;

        return result;
    }

    fn trial(&self) {
        let mut count = 0;
        for i in (0..self.max_segments).into_iter() {
            let temp = self.segments[i].clone();
            if temp.key.get().is_some() {
                count += 1;
            }
        }
        println!("Items in Hash = {}", count);
        println!("===========================");
    }
}

#[derive(Clone)]
struct Bucket<K, V> {
    hop_info: Arc<AtomicU64>,
    key: Arc<AtomicBox<Option<K>>>,
    data: Arc<AtomicBox<Option<V>>>,
}

impl<K, V> Bucket<K, V> {
    #[inline]
    pub fn consume(self) -> Bucket<K, V> {
        self
    }
}

impl<K, V> Default for Bucket<K, V> {
    fn default() -> Self {
        Bucket {
            hop_info: Arc::new(AtomicU64::default()),
            key: Arc::new(AtomicBox::new(None)),
            data: Arc::new(AtomicBox::new(None)),
        }
    }
}

#[cfg(test)]
mod hoptable_tests {
    use super::HOPTable;

    #[test]
    fn hoptable_inserts() {
        let hoptable: HOPTable<String, u64> = HOPTable::new();
        hoptable.insert("Saudade0".to_string(), 1).unwrap();
        hoptable.insert("Saudade1".to_string(), 2).unwrap();
        hoptable.insert("Saudade2".to_string(), 3).unwrap();
        hoptable.insert("Saudade3".to_string(), 4).unwrap();
        hoptable.insert("Saudade4".to_string(), 321321).unwrap();
        hoptable.insert("Saudade5".to_string(), 6).unwrap();

        hoptable.insert("123123".to_string(), 10).unwrap();
        hoptable.insert("1231231".to_string(), 11).unwrap();
        hoptable.insert("1231232".to_string(), 12).unwrap();
        hoptable.insert("1231233".to_string(), 13).unwrap();
        hoptable.insert("1231234".to_string(), 14).unwrap();
        hoptable.insert("1231235".to_string(), 15).unwrap();

        hoptable.trial();
        assert_eq!(hoptable.get(&"Saudade4".to_string()), Some(321321));
    }

    #[test]
    fn hoptable_removes() {
        let hoptable: HOPTable<String, u64> = HOPTable::new();
        hoptable.insert("Saudade0".to_string(), 1).unwrap();
        assert_eq!(hoptable.get(&"Saudade0".to_string()), Some(1));

        hoptable.remove(&"Saudade0".to_string()).unwrap();
        assert_eq!(hoptable.get(&"Saudade0".to_string()), None);
    }

    #[test]
    fn hoptable_upsert() {
        let hoptable: HOPTable<String, u64> = HOPTable::new();
        hoptable.insert("Saudade0".to_string(), 1).unwrap();
        assert_eq!(hoptable.get(&"Saudade0".to_string()), Some(1));

        hoptable.insert("Saudade0".to_string(), 2).unwrap();
        assert_eq!(hoptable.get(&"Saudade0".to_string()), Some(2));
    }

    #[test]
    fn hoptable_nonexistent() {
        let hoptable: HOPTable<u64, u64> = HOPTable::new();
        let k1 = 4856049742280869673_u64;
        let k2 = 2440000773311228611_u64;

        assert_eq!(hoptable.key_index(&k1), hoptable.key_index(&k2));
    }
}
