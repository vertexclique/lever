use super::vars::TVar;
use crate::sync::treiber::TreiberStack;
use std::cell::RefCell;
use std::{
    borrow::{Borrow, BorrowMut},
    collections::HashMap,
    collections::HashSet,
    ptr::NonNull,
};
use std::{fmt, time::Duration};

use super::utils;
use crate::txn::conflicts::*;

use crate::txn::version::{Var, Version};

use std::any::Any;

thread_local! {
    // real: LockVar<T>, T, virt: VersionWrite, VersionWrite
    static LWS: RefCell<HashMap<Version, Version>> = RefCell::new(HashMap::new());
}

// HashMap<*mut LockVar<T>, *mut T>
pub struct WriteSet(HashMap<Version, Version>);

impl WriteSet {
    fn new() -> Self {
        Self(LWS.with(|hs| hs.borrow_mut().clone()))
    }

    pub fn local() -> Self {
        let x = Self::new();
        // dbg!(&x);
        x
    }

    pub fn get<T: Clone + Send + Sync>(&self, e: Var) -> Option<&Version> {
        self.0.get(&Version::Write(e))
    }

    pub fn get_by_stamp<T: 'static + Clone + Send + Sync>(&self, stamp: u64) -> Option<&Version> {
        self.0
            .iter()
            .find(|(tv, _)| {
                let tvar: TVar<T> = utils::version_to_tvar(tv);
                tvar.stamp == stamp
            })
            .map(|g| g.1)
    }

    pub fn put<T: 'static + Clone + Send + Sync>(&mut self, k: Var, v: Var) {
        let kver = Version::Write(k);
        let vver = Version::Write(v);

        self.0.insert(kver, vver);
        LWS.with(|hs| {
            let mut hs = hs.borrow_mut();
            *hs = self.0.clone();
        });
    }

    pub fn try_lock<T: 'static + Clone + Send + Sync>(&mut self, timeout: Duration) -> bool {
        let ts = TreiberStack::<Var>::new();

        for (k, _) in self.0.iter_mut() {
            let read_val = k.read();
            // utils::print_type_of(&read_val);
            let v: Var = utils::direct_convert_ref(&read_val);
            ts.push(v.clone());

            let kv = TVar::new(v.clone());
            if kv.lock.try_lock_for(timeout).is_some() {
                ts.pop();
            // TODO: Not sure if return false or just ignore
            } else {
                return false;
            }
        }

        true
    }

    pub fn get_all<T: 'static + Any + Clone + Send + Sync>(&self) -> Vec<(TVar<T>, T)> {
        self.0
            .iter()
            .map(|(kp, vp)| {
                let k: TVar<T> = utils::version_to_dest(kp);

                let v: T = utils::version_to_dest(vp);

                (k, v)
            })
            .collect()
    }

    pub fn get_all_keys<T: 'static + Clone + Send + Sync>(&self) -> Vec<TVar<T>> {
        self.0
            .keys()
            .map(|p| {
                let v: TVar<T> = utils::version_to_tvar(p);
                v
            })
            .collect::<Vec<TVar<T>>>()
    }

    pub fn unlock<T: 'static + Clone + Send + Sync>(&self) {
        self.get_all_keys().iter().for_each(|_x: &TVar<T>| {
            // TODO: Store guards and drop here for convenience.
            // Normally not needed, anyway.
            // dbg!("Try unlock");
            // unsafe { x.lock.force_unlock_fair(); }
        })
    }

    pub(in crate::txn) fn writes_before<T: 'static + Clone + Send + Sync>(
        &self,
        rev: u64,
    ) -> Vec<Compare> {
        let mut wts = Vec::with_capacity(self.0.len());
        self.0.iter().for_each(|(k, _v)| {
            let var: TVar<T> = utils::version_to_tvar(k);
            if var.modrev < rev {
                let cmp = Compare::new(var.modrev, true, CompareSet::WriteLocal);
                wts.push(cmp);
            }
        });

        wts
    }

    pub fn clear<T: Clone + Send + Sync>(&mut self) {
        // TODO: Drop all here from get_all
        self.0.clear();
        LWS.with(|lws| {
            let mut ws = lws.borrow_mut();
            *ws = self.0.clone();
            // ws.clear();
        })
    }
}

impl fmt::Debug for WriteSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WriteSet").field("ws", &self.0).finish()
    }
}
