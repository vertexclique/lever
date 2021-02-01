use std::sync::atomic::*;
use super::config::*;

pub type StallMrHandle = usize;

pub struct StallMrBatch {
    min_epoch: LFEpoch,

}