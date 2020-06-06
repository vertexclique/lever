use crate::sync::atomics::AtomicBox;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use anyhow::*;

use std::hash::Hash;
use std::hash::{BuildHasher, Hasher};
use std::collections::hash_map::{Iter, Keys, RandomState};

const HOP_RANGE: usize = 1 << 5;
const ADD_RANGE: usize = 1 << 8;
const MAX_SEGMENTS: usize = 1 << 20;
const BUSY: isize = !0;


pub struct HOPTable<K, V, S = RandomState>
where
    K: 'static + PartialEq + Eq + Hash + Clone + Send + Sync + std::fmt::Debug,
    V: 'static + Clone + Send + Sync + std::fmt::Debug,
    S: BuildHasher,
{
    segments: Vec<Arc<AtomicBox<Bucket<K, V>>>>,
    max_segments: usize,
    hash_builder: S,
}

impl<K, V> HOPTable<K, V, RandomState>
where
    K: PartialEq + Eq + Hash + Clone + Send + Sync + std::fmt::Debug,
    V: Clone + Send + Sync + std::fmt::Debug,
{
    pub fn new() -> Self {
        Self::with_capacity(MAX_SEGMENTS + (1 << 8))
    }

    pub fn with_capacity(cap: usize) -> Self {
        Self::with_capacity_and_hasher(cap, RandomState::new())
    }
}

impl<K, V, S> HOPTable<K, V, S>
where
    K: PartialEq + Eq + Hash + Clone + Send + Sync + std::fmt::Debug,
    V: Clone + Send + Sync + std::fmt::Debug,
    S: BuildHasher,
{
    fn with_capacity_and_hasher(cap: usize, hasher: S) -> HOPTable<K, V, S> {
        Self {
            // segments: vec![Arc::new(AtomicBox::new(Bucket::default())); cap],
            segments: (0..cap).into_iter().map(|_| Arc::new(AtomicBox::new(Bucket::default()))).collect(),
            max_segments: cap,
            hash_builder: hasher
        }
    }

    fn hash(&self, key: &K) -> usize {
        let mut hasher = self.hash_builder.build_hasher();
        key.hash(&mut hasher);
        hasher.finish() as usize & (self.max_segments - 1)
    }

    fn seek_segment(&self, key: &K) -> Option<Arc<AtomicBox<Bucket<K, V>>>> {
        let idx = self.key_index(key);
        if idx != !0 {
            Some(self.segments[idx as usize].clone())
        } else {
            None
        }
    }

    #[inline]
    // FIXME: no pub
    pub(crate) fn key_index(&self, k: &K) -> isize {
        let hash = self.hash(k);
        let start_bucket = self.segments[hash].clone();
        let mut mask = 1;
        for i in (0..HOP_RANGE).into_iter() {
            if (mask & start_bucket.get().hop_info
                .load(Ordering::Acquire)) >= 1
            {
                let check_bucket = self.segments[hash + i].clone();
                let keyv = self.extract(check_bucket.get().key.get());
                if Some(k) == keyv.as_ref() {
                    return (hash + i) as isize;
                }
            }
            mask = mask << 1;
        }

        !0
    }


    #[inline]
    fn atomic_remove(&self, k: &K) -> Arc<Option<V>> {
        let hash = self.hash(k);
        let start_bucket = self.segments[hash].clone();

        let remove_bucket_idx = self.key_index(k);
        let distance = remove_bucket_idx as usize - hash;
        if remove_bucket_idx == !0 {
            dbg!(distance);
        }

        if remove_bucket_idx > !0 {
            let remove_bucket = self.segments[remove_bucket_idx as usize].clone();
            let rc = remove_bucket.get().data.get();
            remove_bucket.replace_with(|x| {
                // std::sync::atomic::fence(Ordering::SeqCst);
                let hinfo = x.hop_info.load(Ordering::SeqCst);
                let b = Bucket::default();
                b.hop_info.store(hinfo, Ordering::Relaxed);
                b
            });
            // std::sync::atomic::fence(Ordering::SeqCst);
            let st = start_bucket.get().hop_info.load(Ordering::Acquire);
            start_bucket.get().hop_info.store(st & !(1 << distance), Ordering::SeqCst);
            // println!("REMOVAL COMPLETE {:?} hash: {}", k, remove_bucket_idx);
            return rc;
        }
        println!("REMOVAL NOT COMPLETE {:?} hash: {}, actual_hash: {}", k, remove_bucket_idx, hash);

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
            let val = seg.get().data.get();
            // println!("GET FOUND {:?} hash: {} some: {}", k, self.key_index(k), val.is_some());
            return unsafe { &*Arc::downgrade(&val).as_ptr() }.clone();
        }

        None
    }

    fn extract<A>(&self, val: Arc<A>) -> &A {
        unsafe { &*Arc::downgrade(&val).as_ptr() }
    }

    #[inline]
    fn atomic_insert(&self,
                     start_bucket: &Arc<AtomicBox<Bucket<K, V>>>,
                     free_bucket: &Arc<AtomicBox<Bucket<K, V>>>,
                     k: &K,
                     v: &V,
                     free_distance: usize
    )
    {
        if self.key_index(k) == !0 {
            let sbhi = start_bucket.get().hop_info.load(Ordering::Acquire);
            start_bucket.get().hop_info.store(sbhi | (1 << free_distance), Ordering::SeqCst);
            free_bucket.get().data.replace_with(|_| Some(v.clone()));
            free_bucket.get().key.replace_with(|_| Some(k.clone()));
            // println!("INSERTION COMPLETE {:?} hash: {}", k, self.key_index(k));
        }
        // } else {
        //     println!("SHIT HITS HERE");
        // }
    }

    #[inline]
    pub fn insert(&self, k: K, v: V) -> Result<Arc<Option<V>>> {
        if let Some(_) = self.seek_segment(&k) {
            let _ = self.remove(&k);
        }

        self.new_insert(k, v)
    }

    #[inline]
    fn new_insert(&self, k: K, v: V) -> Result<Arc<Option<V>>> {
        let mut val = 1;

        let hash = self.hash(&k);
        // dbg!(hash);
        let start_bucket = self.segments[hash].clone();
        // let start_bucket = Arc::new(unsafe { &*Arc::downgrade(&self.segments[free_bucket_idx]).as_ptr() }.clone());

        let mut free_bucket_idx = hash;
        let mut free_bucket = self.segments[free_bucket_idx].clone();
        let mut free_distance = 0;

        for _ in (free_distance..ADD_RANGE).into_iter() {
            // dbg!(free_bucket.get().data.get());
            if free_bucket.get().key.get().is_none() {
                // dbg!("breaking");
                break;
            }
            free_bucket_idx += 1;
            // dbg!(free_bucket_idx);
            free_bucket = self.segments[free_bucket_idx].clone();
            // let x = Arc::new(unsafe { &*Arc::downgrade(&self.segments[free_bucket_idx]).as_ptr() }.clone());
        }

        if free_distance < ADD_RANGE {
            while let true = 0 != val {
                if free_distance < HOP_RANGE {
                    self.atomic_insert(&start_bucket, &free_bucket, &k,&v, free_distance);
                    // TODO: Fallback Lock
                    // dbg!("INSERTED");
                    return Ok(Arc::new(Some(v)));
                } else {
                    dbg!("RABBIT HOLE");
                    let closest_binfo = self.find_closer_bucket(free_bucket_idx, free_distance, val);
                    free_distance = closest_binfo[0];
                    val = closest_binfo[1];
                    free_bucket_idx = closest_binfo[2];
                    free_bucket = self.segments[free_bucket_idx].clone();
                }
            }
        }

        Ok(Arc::new(None))
    }

    #[inline]
    fn find_closer_bucket(&self,
                          free_bucket_index: usize,
                          mut free_distance: usize,
                          val: usize
    ) -> [usize; 3] {
        let mut result = [0; 3];
        let mut move_bucket_index = free_bucket_index - (HOP_RANGE - 1);
        let mut move_bucket = self.segments[move_bucket_index].clone();
        for free_dist in (1..HOP_RANGE).rev() {
            let start_hop_info = move_bucket.get().hop_info.load(Ordering::Acquire);
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
                if start_hop_info == move_bucket.get().hop_info.load(Ordering::Acquire) {
                    let new_free_bucket_index = move_bucket_index + move_free_distance as usize;
                    let new_free_bucket = self.segments[new_free_bucket_index].clone();
                    let mbhi = move_bucket.get().hop_info.load(Ordering::Acquire);
                    // Updates move_bucket's hop_info, to indicate the newly inserted bucket
                    move_bucket.get().hop_info.store(mbhi | (1 << free_dist), Ordering::SeqCst);
                    self.segments[free_bucket_index].replace_with(|ex| {
                        let new = new_free_bucket.get();
                        // TODO: No need under some circumstances
                        let ex_hop_info = ex.hop_info.load(Ordering::Acquire);
                        let new_key: Arc<Option<K>> = new.key.get();
                        let new_data: Arc<Option<V>> = new.data.get();

                        let inj = Bucket::default();
                        inj.key.replace_with(|_| {
                            unsafe { &*Arc::downgrade(&new_key).as_ptr() }.clone()
                        });
                        inj.data.replace_with(|_| {
                            unsafe { &*Arc::downgrade(&new_data).as_ptr() }.clone()
                        });
                        inj.hop_info.store(ex_hop_info, Ordering::SeqCst);
                        inj
                    });

                    new_free_bucket.get().key.replace_with(|_| None);
					          new_free_bucket.get().data.replace_with(|_| None);

                    // Updates move_bucket's hop_info, to indicate the deleted bucket
                    move_bucket.get().hop_info.store(move_bucket.get().hop_info.load(Ordering::SeqCst) & !(1 << move_free_distance), Ordering::SeqCst);
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

        self.segments[free_bucket_index].get().key.replace_with(|_| None);
		    result[0] = 0;
		    result[1] = 0;
		    result[2] = 0;

		    return result;
    }

    pub fn trial(&self) {
        let mut count = 0;
        for i in (0..self.max_segments).into_iter() {
            let temp = self.segments[i].clone();
            if temp.get().key.get().is_some() {
                count += 1;
            }
        }
        // println!("Items in Hash = {}", count);
        // println!("===========================");
    }
}

struct Bucket<K, V> {
    hop_info: AtomicU64,
    key: AtomicBox<Option<K>>,
    data: AtomicBox<Option<V>>,
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
            hop_info: AtomicU64::default(),
            key: AtomicBox::new(None),
            data: AtomicBox::new(None)
        }
    }
}



#[cfg(test)]
mod hoptable_tests {
    use super::HOPTable;

    #[test]
    fn hoptable_inserts() {
        let hoptable: HOPTable<String, u64> = HOPTable::new();
        hoptable.insert("Saudade0".to_string(), 1);
        hoptable.insert("Saudade1".to_string(), 2);
        hoptable.insert("Saudade2".to_string(), 3);
        hoptable.insert("Saudade3".to_string(), 4);
        hoptable.insert("Saudade4".to_string(), 321321);
        hoptable.insert("Saudade5".to_string(), 6);

        hoptable.insert("123123".to_string(), 10);
        hoptable.insert("1231231".to_string(), 11);
        hoptable.insert("1231232".to_string(), 12);
        hoptable.insert("1231233".to_string(), 13);
        hoptable.insert("1231234".to_string(), 14);
        hoptable.insert("1231235".to_string(), 15);

        hoptable.trial();
        assert_eq!(hoptable.get(&"Saudade4".to_string()), Some(321321));
    }

    #[test]
    fn hoptable_nonexistent() {
        let hoptable: HOPTable<u64, u64> = HOPTable::new();
        let k1 = 4856049742280869673_u64;
        let k2 = 2440000773311228611_u64;

        assert_eq!(hoptable.key_index(&k1), hoptable.key_index(&k2));
    }
}
