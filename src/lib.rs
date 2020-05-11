// FIXME: Baking still
#![allow(dead_code)]
#![allow(unused_imports)]

// #![feature(core_intrinsics)]
// #![feature(get_mut_unchecked)]

/// Synchronization primitives exposed by Lever
pub mod sync;
/// Transactional in-memory table variations
pub mod table;
/// Transactional primitives and transaction management
pub mod txn;

use std::hash::Hash;
use std::sync::Arc;

use txn::prelude::*;

use crate::table::lotable::LOTable;
use anyhow::*;

///
/// Main management for transactional tables and their management.
///
/// Once get built it can be passed around with simple clone.
///
/// All rules of compute heavy workloads and their limitations apply to Lever's transaction
/// system.
#[derive(Clone)]
pub struct Lever(Arc<TxnManager>);

///
/// Instantiate lever instance
pub fn build() -> Result<Lever> {
    Ok(Lever(TxnManager::manager()))
}

impl Lever {
    ///
    /// Builder method for transactional optimistic, repeatable read in-memory table.
    pub fn make_lo_table<K, V>(_name: String) -> LOTable<K, V>
    where
        K: 'static + PartialEq + Eq + Hash + Clone + Send + Sync,
        V: 'static + Clone + Send + Sync,
    {
        LOTable::new()
    }
}
