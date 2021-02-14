use std::sync::atomic::*;

pub const fn lfatomic_width() -> usize {
    #[cfg(any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "powerpc64",
        target_arch = "mips64"
    ))]
    {
        64_usize
    }
    #[cfg(any(
        target_arch = "x86",
        target_arch = "arm",
        target_arch = "powerpc",
        target_arch = "mips"
    ))]
    {
        32_usize
    }

    #[cfg(not(any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "powerpc64",
        target_arch = "mips64",
        target_arch = "x86",
        target_arch = "arm",
        target_arch = "powerpc",
        target_arch = "mips"
    )))]
    {
        #[cfg(target_pointer_width = "64")]
        {
            64_usize
        }
        #[cfg(target_pointer_width = "32")]
        {
            32_usize
        }
    }
}

pub const fn lfatomic_big_width() -> usize {
    #[cfg(any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "powerpc64",
        target_arch = "mips64"
    ))]
    {
        128_usize
    }
    #[cfg(any(
        target_arch = "x86",
        target_arch = "arm",
        target_arch = "powerpc",
        target_arch = "mips"
    ))]
    {
        64_usize
    }

    #[cfg(not(any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "powerpc64",
        target_arch = "mips64",
        target_arch = "x86",
        target_arch = "arm",
        target_arch = "powerpc",
        target_arch = "mips"
    )))]
    {
        #[cfg(target_pointer_width = "64")]
        {
            64_usize
        }
        #[cfg(target_pointer_width = "32")]
        {
            32_usize
        }
    }
}

pub const fn lfatomic_log2() -> usize {
    #[cfg(any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "powerpc64",
        target_arch = "mips64"
    ))]
    {
        3_usize
    }
    #[cfg(any(
        target_arch = "x86",
        target_arch = "arm",
        target_arch = "powerpc",
        target_arch = "mips"
    ))]
    {
        2_usize
    }

    #[cfg(not(any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "powerpc64",
        target_arch = "mips64",
        target_arch = "x86",
        target_arch = "arm",
        target_arch = "powerpc",
        target_arch = "mips"
    )))]
    {
        #[cfg(target_pointer_width = "64")]
        {
            3_usize
        }
        #[cfg(target_pointer_width = "32")]
        {
            2_usize
        }
    }
}

pub const fn load_lohi_split<D>() -> bool {
    #[cfg(any(
        target_arch = "x86",
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "arm"
    ))]
    {
        (std::mem::size_of::<D>() * 8) > lfatomic_width()
    }

    #[cfg(not(any(
        target_arch = "x86",
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "arm",
    )))]
    {
        false
    }
}

///
/// Note: There is no `lo` compare `lo` and `hi` swap single compare exchange existing in any current architecture that Rust supports.
/// IA-64 was the only one of them and LLVM support for IA-64 has never been officially merged, thus Rust haven't had it at all.
pub const fn cmpxchg_lohi_split<D>() -> bool {
    false
}

//////////////////////
// Epoch type aliases for 64-bit archs
//////////////////////

#[cfg(any(
    target_arch = "x86_64",
    target_arch = "aarch64",
    target_arch = "powerpc64",
    target_arch = "mips64"
))]
pub type LFEpoch = u64;
#[cfg(any(
    target_arch = "x86_64",
    target_arch = "aarch64",
    target_arch = "powerpc64",
    target_arch = "mips64"
))]
pub type LFEpochSigned = i64;
#[cfg(any(
    target_arch = "x86_64",
    target_arch = "aarch64",
    target_arch = "powerpc64",
    target_arch = "mips64"
))]
pub type LFAtomic = AtomicU64;
#[cfg(any(
target_arch = "x86_64",
target_arch = "aarch64",
target_arch = "powerpc64",
target_arch = "mips64"
))]
pub type AtomicLFEpoch = AtomicU64;
#[cfg(any(
target_arch = "x86_64",
target_arch = "aarch64",
target_arch = "powerpc64",
target_arch = "mips64"
))]
pub type AtomicGenericT = AtomicUsize;
// pub type LFAtomicBig = 128BitDCASSingleCompareDoubleSwap;

//////////////////////
// Epoch type aliases for 32-bit archs
//////////////////////

// x86 should still use 64-bit vars
#[cfg(target_arch = "x86")]
pub type LFEpoch = u64;
#[cfg(target_arch = "x86")]
pub type LFEpochSigned = i64;

// Rest of the architectures
#[cfg(any(target_arch = "arm", target_arch = "powerpc", target_arch = "mips"))]
pub type LFEpoch = u32;
#[cfg(any(target_arch = "arm", target_arch = "powerpc", target_arch = "mips"))]
pub type LFEpochSigned = i32;
#[cfg(any(
    target_arch = "x86",
    target_arch = "arm",
    target_arch = "powerpc",
    target_arch = "mips"
))]
pub type LFAtomic = AtomicU32;
// pub type LFAtomicBig = 128BitDCASSingleCompareDoubleSwap;

//////////////////////
// Epoch type aliases for all the other architectures. Equivalent to the target pointer size.
//////////////////////

#[cfg(not(any(
    target_arch = "x86_64",
    target_arch = "aarch64",
    target_arch = "powerpc64",
    target_arch = "mips64",
    target_arch = "x86",
    target_arch = "arm",
    target_arch = "powerpc",
    target_arch = "mips"
)))]
pub type LFEpoch = usize;
#[cfg(not(any(
    target_arch = "x86_64",
    target_arch = "aarch64",
    target_arch = "powerpc64",
    target_arch = "mips64",
    target_arch = "x86",
    target_arch = "arm",
    target_arch = "powerpc",
    target_arch = "mips"
)))]
pub type LFEpochSigned = isize;
#[cfg(not(any(
    target_arch = "x86_64",
    target_arch = "aarch64",
    target_arch = "powerpc64",
    target_arch = "mips64",
    target_arch = "x86",
    target_arch = "arm",
    target_arch = "powerpc",
    target_arch = "mips"
)))]
pub type LFAtomic = AtomicUsize;
// pub type LFAtomicBig = 128BitDCASSingleCompareDoubleSwap;
