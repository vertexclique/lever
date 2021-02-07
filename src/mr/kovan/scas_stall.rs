use super::config::*;
use cuneiform_fields::alignas::AlignAs16;
use cuneiform_fields::hermetic::HermeticPadding;
use std::mem::ManuallyDrop;
use std::sync::atomic::*;

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

mod scas_stall_tests {
    use super::*;

    #[test]
    fn test_scan_mr_node() {
        assert_eq!(32, std::mem::size_of::<MrLink>());
    }
}
