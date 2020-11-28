// Behind the feature gates
#![cfg_attr(feature = "hw", feature(stdsimd))]
#![cfg_attr(feature = "hw", feature(llvm_asm))]
// FIXME: Baking still
#![allow(dead_code)]
#![allow(unused_imports)]
// #![feature(core_intrinsics)]
// #![feature(get_mut_unchecked)]

//!
//! Lever is a library for writing transactional systems (esp. for in-memory data). It consists of various parts:
//! * `sync`: Synchronization primitives for transactional systems
//! * `table`: Various KV table kinds backed by transactional algorithms
//! * `txn`: Transactional primitives and management
//!
//! Lever is using MVCC model to manage concurrency. It supplies building blocks for in-memory data stores for
//! transactional endpoints, databases and systems. Unblocked execution path is main aim for lever while
//! not sacrificing failover mechanisms.
//!
//! Lever provides STM, lock-free, wait-free synchronization primitives and various other tools to facilitate writing
//! transactional in-memory systems.
//!
//! Lever is alpha stage software.

#![doc(
    html_logo_url = "https://raw.githubusercontent.com/vertexclique/lever/master/img/lever-square.png"
)]

/// Indexes and lookup structures
pub mod index;
/// Statistics related structures
pub mod stats;
/// Synchronization primitives
pub mod sync;
/// Transactional in-memory table variations
pub mod table;
/// Transactional primitives and transaction management
pub mod txn;

/// Allocation helpers
mod alloc;

/// Hardware transactional memory
mod htm;

use std::hash::Hash;
use std::sync::Arc;

///
/// Prelude of lever
pub mod prelude {
    pub use crate::sync::prelude::*;
    pub use crate::table::prelude::*;
    pub use crate::txn::prelude::*;
}

use crate::table::lotable::LOTable;
use crate::txn::transact::TxnManager;

use anyhow::*;

///
/// Main management struct for transaction management.
///
/// Once get built it can be passed around with simple clone.
///
/// All rules of compute heavy workloads and their limitations apply to Lever's transaction
/// system.
#[derive(Clone)]
pub struct Lever(Arc<TxnManager>);

///
/// Instantiate lever instance
pub fn lever() -> Lever {
    Lever(TxnManager::manager())
}

impl Lever {
    ///
    /// Builder method for transactional optimistic, repeatable read in-memory table.
    pub fn new_lotable<K, V>(&self) -> LOTable<K, V>
    where
        K: 'static + PartialEq + Eq + Hash + Clone + Send + Sync + Ord,
        V: 'static + Clone + Send + Sync,
    {
        LOTable::new()
    }

    ///
    /// Get global transaction manager
    pub fn manager(&self) -> Arc<TxnManager> {
        self.0.clone()
    }
}
