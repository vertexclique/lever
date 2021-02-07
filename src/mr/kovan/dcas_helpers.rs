use cuneiform::*;

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

        pub const fn lf_merger(l: $dt, r: $dt) -> $dt {
            // LEA optimization
            #[cfg(any(target_arch = "x86", target_arch = "x86_64",))]
            {
                l + r
            }

            #[cfg(not(any(target_arch = "x86", target_arch = "x86_64",)))]
            {
                l | r
            }
        }
    };
}

// Ptr index for double-width types.
#[cfg(target_endian = "little")]
pub const LFREF_LINK: usize = 0;
#[cfg(target_endian = "big")]
pub const LFREF_LINK: usize = 1;

#[boundary_size]
type BS = ();

pub const fn lf_cache_bytes() -> usize {
    BOUNDARY_SIZE as _
}

#[cfg(test)]
mod dcas_helpers_tests {
    use crate::mr::kovan::dcas_helpers::*;

    #[test]
    fn test_check_cache_line_cas_boundary() {
        #[cfg(target_arch = "x86_64")]
        assert_eq!(lf_cache_bytes(), 128_usize);

        assert!(true);
    }
}
