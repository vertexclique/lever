use anyhow::*;
use std::alloc::Layout;

pub(crate) fn bucket_allocate_cont<T>(buckets: usize) -> Result<Vec<T>> {
    // debug_assert!((buckets != 0) && ((buckets & (buckets - 1)) == 0), "Capacity should be power of 2");

    // Array of buckets
    let data = Layout::array::<T>(buckets)?;

    unsafe {
        let p = std::alloc::alloc_zeroed(data);

        Ok(Vec::<T>::from_raw_parts(p as *mut T, 0, buckets))
    }
}

pub(crate) fn bucket_alloc<T>(init_cap: usize) -> Vec<T>
where
    T: Default
{
    let data = Layout::array::<T>(init_cap).unwrap();
    let p = unsafe { std::alloc::alloc_zeroed(data) as *mut T };
    unsafe {
        (0..init_cap)
            .for_each(|i| {
                std::ptr::write(p.offset(i as isize), T::default());
            });
        Vec::from_raw_parts(p, init_cap, init_cap)
    }
}
