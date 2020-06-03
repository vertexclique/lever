//#[cfg_attr(hw, attr)]

// Intel RTM
#[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), feature = "hw"))]
use super::x86_64 as htm;

// Aarch64 TME
#[cfg(all(target_arch = "aarch64", feature = "hw"))]
use super::aarch64 as htm;

/// HTM support
use htm::*;

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
