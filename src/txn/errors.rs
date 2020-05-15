use anyhow::*;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TxnErrorType {
    #[error("Retry mechanism triggered")]
    Retry = 0,
    #[error("Abort triggered")]
    Abort = 1,
}

pub type TxnResult<T> = anyhow::Result<T, TxnErrorType>;
