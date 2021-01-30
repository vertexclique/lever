//////////////////////////
// Ref counting
//////////////////////////

macro_rules! lfref_impl {
    ($dt:ty) => {
        pub const fn lfref_shift() -> usize {
            std::mem::size_of::<$dt>() * 4_usize
        }

        pub const fn lfptr_shift() -> usize {
            0_usize
        }

        pub const fn lfref_mask() -> $dt {
            !(0_usize << (std::mem::size_of::<$dt>() * 4_usize))
        }

        pub const fn lfref_step() -> $dt {
            !(1_usize << (std::mem::size_of::<$dt>() * 4_usize))
        }
    };
}

// Ptr index for double-width types.
#[cfg(target_endian = "little")]
pub const LFREF_LINK: usize = 0;
#[cfg(target_endian = "big")]
pub const LFREF_LINK: usize = 1;
