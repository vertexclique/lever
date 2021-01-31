//#[cfg_attr(hw, attr)]

// Intel RTM
#[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), feature = "hw"))]
use super::x86_64 as htm;

// Aarch64 TME
#[cfg(all(target_arch = "aarch64", feature = "hw"))]
use super::aarch64 as htm;

use crate::txn::errors::{TxnErrorType, TxnResult};
use crate::txn::transact::TxnManager;
/// HTM support
use htm::*;
use log::*;
use std::{any::Any, marker::PhantomData};

///
/// Unified interface for TM operations at hw level
pub(super) trait Ops {
    ///
    /// Runtime: TM hw feature existence
    fn cpu_support(&self) -> bool;

    ///
    /// Begin transactional region
    fn begin(&self) -> HwTxBeginCode;

    ///
    /// Abort transactional region
    ///
    /// # Arguments
    /// * `reason_code` - Abort reason code for reason accepting archs.
    fn abort(&self, reason_code: &HwTxAbortCode) -> !;

    ///
    /// Test if we're in txn region
    fn test(&self) -> HwTxTestCode;

    ///
    /// Commit or end the transactional region
    fn commit(&self);
}

pub struct HwTxn();

impl HwTxn {
    ///
    /// Initiate hardware transaction with given closure.
    pub fn begin<F, R>(&self, mut f: F) -> TxnResult<R>
    where
        F: FnMut(&mut HTM) -> R,
        R: 'static + Any + Clone + Send + Sync,
    {
        let mut htm = HTM();
        let bcode = htm.begin();
        let r = loop {
            if bcode.started() {
                let res = f(&mut htm);
                htm.commit();
                break Ok(res);
            } else {
                let reason = if bcode.started() == false {
                    "NOT_STARTED"
                } else if bcode.capacity() {
                    "CAPACITY"
                } else if bcode.abort() {
                    "ABORTED"
                } else if bcode.retry() {
                    "RETRY_POSSIBLE"
                } else if bcode.conflict() {
                    "CONFLICT"
                } else if bcode.debug() {
                    "DEBUG"
                } else {
                    "CAUSE_UNKNOWN"
                };
                debug!("htx::failure::cause::{}", reason);
                // TODO: htm.abort(&HwTxAbortCode::UserlandAbort);
                break Err(TxnErrorType::Abort);
            }
        };

        r
    }
}

#[cfg(test)]
mod lever_hwtxn_test {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::{AtomicPtr, Ordering};

    pub fn swallow<T>(d: T) -> T {
        unsafe {
            llvm_asm!("" : : "r"(&d));
            d
        }
    }

    #[test]
    fn hwtxn_start() {
        std::thread::spawn(move || {
            let hwtxn = HwTxn();
            let data = hwtxn.begin(|_htm| 1 + 2);

            assert_eq!(data.unwrap(), 3);
        });
    }

    #[test]
    fn hwtxn_start_arc() {
        let x = AtomicUsize::new(100);
        std::thread::spawn(move || {
            let hwtxn = HwTxn();
            let _data = hwtxn.begin(|_htm| x.fetch_add(1, Ordering::Relaxed));
        });
    }

    #[test]
    #[ignore]
    fn hwtxn_block_test() {
        let x = 123;
        std::thread::spawn(move || {
            let htm = HTM();
            assert_eq!(true, htm.begin().started());
            let _ = x + 1;
            assert_eq!(true, htm.test().in_txn());
            htm.abort(&HwTxAbortCode::UserlandAbort);
        });

        std::thread::spawn(move || {
            let htm = HTM();
            std::thread::sleep(std::time::Duration::from_millis(10));
            assert_eq!(true, htm.begin().started());
            assert_eq!(true, htm.test().in_txn());
            htm.commit();
            assert_eq!(false, htm.test().in_txn());
        });
    }

    #[test]
    fn hwtxn_capacity_check() {
        use std::mem;

        const CACHE_LINE_SIZE: usize = 64 / mem::size_of::<usize>();

        let mut data = vec![0usize; 1_000_000];
        let mut capacity = 0;
        let end = data.len() / CACHE_LINE_SIZE;
        for i in (0..end).rev() {
            data[i * CACHE_LINE_SIZE] = data[i * CACHE_LINE_SIZE].wrapping_add(1);
            swallow(&mut data[i * CACHE_LINE_SIZE]);
        }
        for max in 0..end {
            let _fail_count = 0;
            let hwtxn = HwTxn();
            let _data = hwtxn.begin(|_htm| {
                for i in 0..max {
                    let elem = unsafe { data.get_unchecked_mut(i * CACHE_LINE_SIZE) };
                    *elem = elem.wrapping_add(1);
                }
            });
            capacity = max;
        }
        swallow(&mut data);
        println!("sum: {}", data.iter().sum::<usize>());

        println!(
            "Capacity: {}",
            capacity * mem::size_of::<usize>() * CACHE_LINE_SIZE
        );
    }
}
