use std::convert::TryFrom;
use std::ops::{Deref, DerefMut};
use std::ptr::NonNull;
use std::sync::Arc;

pub struct ArcUnique<T: Clone>(NonNull<T>);

impl<T: Clone> From<T> for ArcUnique<T> {
    fn from(value: T) -> Self {
        unsafe {
            Self(NonNull::new_unchecked(
                Arc::into_raw(Arc::new(value)) as *mut T
            ))
        }
    }
}

impl<T: Clone> TryFrom<Arc<T>> for ArcUnique<T> {
    type Error = Arc<T>;

    fn try_from(mut arc: Arc<T>) -> Result<Self, Arc<T>> {
        if Arc::get_mut(&mut arc).is_some() {
            unsafe { Ok(Self(NonNull::new_unchecked(Arc::into_raw(arc) as *mut T))) }
        } else {
            Err(arc)
        }
    }
}

impl<T: Clone> Into<Arc<T>> for ArcUnique<T> {
    fn into(self) -> Arc<T> {
        unsafe { Arc::from_raw(self.0.as_ptr()) }
    }
}

impl<T: Clone> Deref for ArcUnique<T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { self.0.as_ref() }
    }
}

impl<T: Clone> DerefMut for ArcUnique<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { self.0.as_mut() }
    }
}
