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
