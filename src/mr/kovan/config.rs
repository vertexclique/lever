

pub const fn lfatomic_width() -> usize {
    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "powerpc64", target_arch = "mips64"))] {
        64_usize
    }
    #[cfg(any(target_arch = "x86", target_arch = "arm", target_arch = "powerpc", target_arch = "mips"))] {
        32_usize
    }

    #[cfg(
    not(
    any(
    target_arch = "x86_64",
    target_arch = "aarch64",
    target_arch = "powerpc64",
    target_arch = "mips64",
    target_arch = "x86",
    target_arch = "arm",
    target_arch = "powerpc",
    target_arch = "mips"
    )
    )
    )]
        {
            #[cfg(target_pointer_width = "64")] {
                64_usize
            }
            #[cfg(target_pointer_width = "32")] {
                32_usize
            }
        }
}

pub const fn lfatomic_big_width() -> usize {
    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "powerpc64", target_arch = "mips64"))] {
        128_usize
    }
    #[cfg(any(target_arch = "x86", target_arch = "arm", target_arch = "powerpc", target_arch = "mips"))] {
        64_usize
    }

    #[cfg(
        not(
            any(
            target_arch = "x86_64",
            target_arch = "aarch64",
            target_arch = "powerpc64",
            target_arch = "mips64",
            target_arch = "x86",
            target_arch = "arm",
            target_arch = "powerpc",
            target_arch = "mips"
            )
        )
    )]
    {
        #[cfg(target_pointer_width = "64")] {
            64_usize
        }
        #[cfg(target_pointer_width = "32")] {
            32_usize
        }
    }
}