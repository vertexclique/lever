use super::ops::*;

#[cfg(target_arch = "x86")]
use std::arch::x86::{
    _xabort, _xabort_code, _xbegin, _xend, _xtest, _XABORT_CAPACITY, _XABORT_CONFLICT,
    _XABORT_DEBUG, _XABORT_EXPLICIT, _XABORT_RETRY, _XBEGIN_STARTED,
};
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::{
    _xabort, _xabort_code, _xbegin, _xend, _xtest, _XABORT_CAPACITY, _XABORT_CONFLICT,
    _XABORT_DEBUG, _XABORT_EXPLICIT, _XABORT_RETRY, _XBEGIN_STARTED,
};

/// Return code from _xbegin()
pub struct HwTxBeginCode(u32);

impl HwTxBeginCode {
    #[inline]
    pub fn started(&self) -> bool {
        self.0 == _XBEGIN_STARTED
    }

    #[inline]
    pub fn abort(&self) -> bool {
        self.0 & _XABORT_EXPLICIT != 0 && !self.started()
    }

    #[inline]
    pub fn retry(&self) -> bool {
        self.0 & _XABORT_RETRY != 0 && !self.started()
    }

    #[inline]
    pub fn conflict(&self) -> bool {
        self.0 & _XABORT_CONFLICT != 0 && !self.started()
    }

    #[inline]
    pub fn capacity(&self) -> bool {
        self.0 & _XABORT_CAPACITY != 0 && !self.started()
    }

    #[inline]
    pub fn debug(&self) -> bool {
        self.0 & _XABORT_DEBUG != 0 && !self.started()
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
        *self as u32 == _xabort_code(*other as u32)
    }
}

/// Return code from __ttest()
pub struct HwTxTestCode(u8);

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
    const OVERHAUL: u32 = HwTxAbortCode::Overhaul as u32;
    const USERLAND_ABORT: u32 = HwTxAbortCode::UserlandAbort as u32;
}

impl Ops for HTM {
    fn begin(&self) -> HwTxBeginCode {
        unsafe { HwTxBeginCode(_xbegin()) }
    }
    fn abort(&self, reason_code: &HwTxAbortCode) -> ! {
        unsafe {
            match reason_code {
                HwTxAbortCode::Overhaul => _xabort(HTM::OVERHAUL),
                HwTxAbortCode::UserlandAbort => _xabort(HTM::USERLAND_ABORT),
            }
            std::hint::unreachable_unchecked()
        }
    }
    fn test(&self) -> HwTxTestCode {
        unsafe { HwTxTestCode(_xtest()) }
    }
    fn commit(&self) {
        unsafe { _xend() }
    }
    fn cpu_support(&self) -> bool {
        std::is_x86_feature_detected!("rtm")
    }
}
