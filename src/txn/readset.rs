use super::utils;
use super::vars::TVar;
use crate::txn::conflicts::*;
use crate::txn::version::{Var, Version};

use std::cell::RefCell;
use std::{
    borrow::{Borrow, BorrowMut},
    collections::HashSet,
};

thread_local! {
    // real: Arc<TVar<T>>
    // virtual: Var
    pub(crate) static LRS: RefCell<HashSet<Version>> = RefCell::new(HashSet::new());
}

// HashSet<*mut LockVar<T>>
pub struct ReadSet(pub HashSet<Version>);

impl ReadSet {
    fn new() -> Self {
        Self(LRS.with(|hs| {
            let hs = hs.borrow();
            hs.clone()
        }))
    }

    pub fn local() -> Self {
        Self::new()
    }

    pub fn get_all(&self) -> Vec<Var> {
        self.0.iter().map(|e| e.read()).collect()
    }

    pub fn get_all_versions(&self) -> Vec<&Version> {
        self.0.iter().collect()
    }

    pub fn get<T: Clone>(&self, seek: Version) -> Option<Var> {
        self.0.iter().find(|x| **x == seek).map(|f| f.read())
    }

    pub fn add(mut self, e: Var) {
        let v = Version::Read(e);
        self.0.insert(v);

        LRS.with(|hs| {
            let mut hs = hs.borrow_mut();
            *hs = self.0.clone();
        });
    }

    pub(in crate::txn) fn cmps<T: 'static + Clone + Sync + Send>(&self) -> Vec<Compare> {
        let mut cmset = Vec::with_capacity(self.0.len());
        self.0.iter().for_each(|v| {
            let var: TVar<T> = utils::version_to_tvar(v);
            let cmp = Compare::new(var.modrev, var.modrev == var.stamp, CompareSet::ReadLocal);

            cmset.push(cmp);
        });

        cmset
    }

    /// First stamp
    pub(crate) fn first<T: 'static + Clone + Sync + Send>(&self) -> u64 {
        let mut min_stamp = u64::MAX;
        for x in self.0.iter() {
            let v: TVar<T> = utils::version_to_tvar(x);
            // dbg!(v.stamp);
            if v.stamp < min_stamp {
                min_stamp = v.stamp;
            }
        }
        // dbg!("=======");
        min_stamp
    }

    pub fn clear(&mut self) {
        // TODO: Drop all here from get_all
        self.0.clear();
        LRS.with(|hs| {
            let mut hs = hs.borrow_mut();
            *hs = self.0.clone();
        });
    }
}
