use super::ops::*;

#[cfg(target_arch = "aarch64")]
use std::arch::aarch64::{
    __tstart, __ttest, __tcommit, __tcancel,
    _TMSTART_SUCCESS, _TMFAILURE_TRIVIAL, _TMFAILURE_SIZE, _TMFAILURE_RTRY, _TMFAILURE_REASON, _TMFAILURE_NEST, _TMFAILURE_MEM, _TMFAILURE_INT, _TMFAILURE_IMP,
    _TMFAILURE_ERR, _TMFAILURE_DBG, _TMFAILURE_CNCL
};

/// Return code from __tstart()
pub struct HwTxBeginCode(u64);

impl HwTxBeginCode {
    #[inline]
    pub fn started(&self) -> bool {
        self.0 == _TMSTART_SUCCESS
    }

    #[inline]
    pub fn abort(&self) -> bool {
        self.0 & _TMFAILURE_CNCL != 0 && !self.started()
    }

    #[inline]
    pub fn retry(&self) -> bool {
        self.0 & _TMFAILURE_RTRY != 0 && !self.started()
    }

    #[inline]
    pub fn conflict(&self) -> bool {
        self.0 & _TMFAILURE_MEM != 0 && !self.started()
    }

    #[inline]
    pub fn capacity(&self) -> bool {
        self.0 & _TMFAILURE_SIZE != 0 && !self.started()
    }

    /// Aarch64 specific
    #[inline]
    pub fn nest_exceeded(&self) -> bool {
        self.0 & _TMFAILURE_NEST != 0 && !self.started()
    }

    /// Aarch64 specific
    #[inline]
    pub fn trivial_exec(&self) -> bool {
        self.0 & _TMFAILURE_TRIVIAL != 0 && !self.started()
    }

    /// Aarch64 specific
    #[inline]
    pub fn non_permissible(&self) -> bool {
        self.0 & _TMFAILURE_ERR != 0 && !self.started()
    }

    /// Aarch64 specific
    #[inline]
    pub fn interrupted(&self) -> bool {
        self.0 & _TMFAILURE_INT != 0 && !self.started()
    }

    /// Aarch64 specific
    #[inline]
    pub fn fallback_failure(&self) -> bool {
        self.0 & _TMFAILURE_IMP != 0 && !self.started()
    }

    #[inline]
    pub fn debug(&self) -> bool {
        self.0 & _TMFAILURE_DBG != 0 && !self.started()
    }
}

/// most significant 8 bits
#[derive(Copy, Clone)]
pub enum HwTxAbortCode {
    Overhaul = 1 << 0,
    UserlandAbort = 1 << 1,
}

impl PartialEq for HwTxAbortCode {
    fn eq(&self, other: &HwTxAbortCode) -> bool {
        *self as u64 == HTM::_tcancel_code(*other as u64, true)
    }
}

/// Return code from __ttest()
pub struct HwTxTestCode(u64);

impl HwTxTestCode {
    #[inline]
    pub fn in_txn(&self) -> bool {
        self.0 != 0
    }

    #[inline]
    fn depth(&self) -> usize {
        self.0 as usize
    }
}

pub struct HTM();

impl HTM {
    const OVERHAUL: u64 = HwTxAbortCode::Overhaul as u64;
    const USERLAND_ABORT: u64 = HwTxAbortCode::UserlandAbort as u64;

    /// Encodes cancellation reason, which is the parameter passed to [`__tcancel`]
    /// Takes cancellation reason flags and retry-ability.
    #[inline]
    pub const fn _tcancel_code(reason: u64, retryable: bool) -> u64 {
        ((retryable as i64) << 15 | (reason & _TMFAILURE_REASON) as i64) as u64
    }
}

impl Ops for HTM {
    fn begin(&self) -> HwTxBeginCode {
        unsafe { HwTxBeginCode(__tstart()) }
    }

    fn abort(&self, reason_code: &HwTxAbortCode) -> ! {
        // TODO: Pass retryable as argument?
        unsafe {
            match reason_code {
                HwTxAbortCode::Overhaul => __tcancel(HTM::_tcancel_code(HTM::OVERHAUL, true)),
                HwTxAbortCode::UserlandAbort => __tcancel(HTM::_tcancel_code(HTM::USERLAND_ABORT, true)),
            }
            std::hint::unreachable_unchecked()
        }
    }

    fn test(&self) -> HwTxTestCode {
        unsafe { HwTxTestCode(__ttest()) }
    }
    fn commit(&self) {
        unsafe { __tcommit() }
    }
    fn cpu_support(&self) -> bool {
        std::is_aarch64_feature_detected!("tme")
    }
}
