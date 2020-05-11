use super::transact::{TransactionConcurrency, TransactionIsolation};

pub(crate) const DEFAULT_TX_TIMEOUT: usize = 0_usize;
pub(crate) const DEFAULT_TX_SERIALIZABLE_ENABLED: bool = false;
pub(crate) const DEFAULT_TX_CONCURRENCY: TransactionConcurrency =
    TransactionConcurrency::Pessimistic;
pub(crate) const DEFAULT_TX_ISOLATION: TransactionIsolation = TransactionIsolation::RepeatableRead;
