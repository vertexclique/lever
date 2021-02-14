use super::config::*;
use cuneiform_fields::alignas::AlignAs16;
use cuneiform_fields::hermetic::HermeticPadding;
use std::mem::{ManuallyDrop, MaybeUninit};
use std::sync::atomic::*;
use cuneiform_fields::arch::ArchPadding;
use crate::mr::kovan::dcas_helpers::lf_cache_bytes;

//////////////////////////
// Common implementation for single cas, can be altered by GenericT size.
//////////////////////////

///
/// Generic type for single CAS implementation
pub type GenericT = usize;

pub type StallMrHandle = usize;

///
/// Batch to track allocations
#[repr(C)]
pub struct StallMrBatch {
    min_epoch: LFEpoch,
    first: GenericT,
    last: GenericT,
    counter: GenericT,
}

///
/// Reference list block
#[repr(C)]
union MrRefBlock {
    refs: ManuallyDrop<LFAtomic>,
    batch_next: GenericT,
}

#[repr(C)]
struct MrLink {
    next: AlignAs16<GenericT>,
    batch_link: GenericT,
    ref_block: MrRefBlock,
}

#[repr(C)]
union StallMrBlock {
    mr_link: ManuallyDrop<MrLink>,
    birth_epoch: LFEpoch,
}

#[repr(C)]
struct StallMrNode(StallMrBlock);

// Actual implementation following

#[repr(C)]
struct StallMrVector {
    // Atomic<GenericT> is a variant that was expected.
    head: ArchPadding<AtomicGenericT>,
    access: ArchPadding<AtomicLFEpoch>,
    _priv: ArchPadding<MaybeUninit<[u8; lf_cache_bytes()]>>
}

#[repr(C)]
struct StallMr;

#[repr(C)]
struct StallMrFree {
    mr: *mut StallMr,
    mr_node: *mut StallMrNode
}

impl StallMr {
    #[inline(always)]
    pub fn link(&self, size: usize) -> GenericT {
        todo!("link")
    }

    #[inline(always)]
    pub fn ack(&self, size: usize) -> GenericT {
        todo!("ack")
    }
}

// extern "C" {
//     pub type lfbsmrw;
//     pub type lfbsmrw_node;
// }
// pub type lfbsmrw_free_t
// =
// Option<unsafe extern "C" fn(_: *mut lfbsmrw, _: *mut lfbsmrw_node) -> ()>;
// unsafe fn main_0() -> libc::c_int {
//     let mut a: lfbsmrw_free_t = None;
//     return 0;
// }

// Tests
mod scas_stall_tests {
    use super::*;

    #[test]
    fn test_scan_mr_node() {
        assert_eq!(32, std::mem::size_of::<MrLink>());
    }
}
