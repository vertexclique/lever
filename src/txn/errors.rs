use std::result;
use thiserror::Error;

#[derive(Clone, Error, Debug)]
pub enum TxnErrorType {
    #[error("Retry mechanism triggered")]
    Retry,
    #[error("Abort triggered")]
    Abort,
    #[error("Txn retry with: {0}")]
    RetryWithContext(String),
    #[error("Txn abort with: {0}")]
    AbortWithContext(String)
}

pub type TxnResult<T> = result::Result<T, TxnErrorType>;
