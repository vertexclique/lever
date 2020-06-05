#[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), feature = "hw"))]
mod x86_64;

#[cfg(all(target_arch = "aarch64", feature = "hw"))]
mod aarch64;

/// Architecture operations
#[cfg(feature = "hw")]
pub mod ops;
