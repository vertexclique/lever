use super::config::*;
use super::dcas_helpers::lf_cache_bytes;
use cuneiform_fields::alignas::AlignAs16;
use cuneiform_fields::hermetic::HermeticPadding;
use std::mem::{ManuallyDrop, MaybeUninit};
use std::sync::atomic::*;
use cuneiform_fields::arch::ArchPadding;

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
struct StallMr {
    global: ArchPadding<AtomicLFEpoch>,
    vectors: Vec<StallMrVector>
}

#[repr(C)]
struct StallMrFree {
    mr: *mut StallMr,
    mr_node: *mut StallMrNode
}

impl StallMr {
    #[inline(always)]
    pub fn init(&mut self) {
        self.vectors.iter_mut().for_each(|r| {
            r.head = ArchPadding::new(AtomicGenericT::default());
            r.access = ArchPadding::new(AtomicLFEpoch::default());
        });
        self.global = ArchPadding::new(AtomicLFEpoch::default());
    }

    #[inline(always)]
    pub fn link(&self, vec: usize) -> GenericT {
        self.vectors[vec].access.load(Ordering::Acquire) as GenericT & !0x1
    }

    #[inline(always)]
    pub fn access(&self, vec: usize, access: LFEpoch, epoch: LFEpoch) -> LFEpoch {
        self.vectors[vec].access.store(epoch as _, Ordering::SeqCst);
        epoch
    }

    #[inline(always)]
    pub fn enter(&self, vec: usize, order: usize, smr: &mut StallMrHandle, base: *const (), )

    #[inline(always)]
    pub fn ack(&self, size: usize) -> GenericT {
        unimplemented!("ack")
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
