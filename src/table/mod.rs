/// Lever Transactional Table implementation with [Optimistic](crate::txn::transact::TransactionConcurrency::Optimistic)
/// concurrency and [RepeatableRead](crate::txn::transact::TransactionIsolation::RepeatableRead) isolation.
pub mod lotable;
#[doc(hidden)]
pub mod ltable;

#[cfg(feature = "nightly")]
pub mod hoptable;

/// Prelude for transactional KV table implementations
pub mod prelude {
    #[cfg(feature = "nightly")]
    pub use super::hoptable::*;

    pub use super::lotable::*;
}
