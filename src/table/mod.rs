/// Lever Transactional Table implementation with [Optimistic](crate::txn::transact::TransactionConcurrency::Optimistic)
/// concurrency and [RepeatableRead](crate::txn::transact::TransactionIsolation::RepeatableRead) isolation.
pub mod lotable;
#[doc(hidden)]
pub mod ltable;

/// Prelude for transactional KV table implementations
pub mod prelude {
    pub use super::lotable::*;
}
