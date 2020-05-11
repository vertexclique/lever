use std::any::Any;
use std::fmt;
use std::fmt::Formatter;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData as marker;
use std::ptr::NonNull;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

pub type Var = Arc<dyn Any + Send + Sync>;

#[derive(Clone)]
pub enum Version {
    Read(Var),
    Write(Var),
}

impl Version {
    pub fn extract(&self) -> &Var {
        match self {
            Version::Read(x) => x,
            Version::Write(x) => x,
        }
    }

    pub fn read(&self) -> Var {
        return match &*self {
            &Version::Read(ref v) | &Version::Write(ref v) => v.clone(),
        };
    }

    pub fn write(&mut self, w: Var) {
        *self = match self.clone() {
            Version::Write(_) => Version::Write(w),
            // TODO: Not sure
            _ => Version::Write(w),
        };
    }
}

impl fmt::Debug for Version {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Version").field("var", &self).finish()
    }
}

impl Hash for Version {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let x: ArcLayout<dyn Any + Send + Sync> = unsafe { std::mem::transmute_copy(&self.read()) };

        x.ptr.as_ptr().hash(state);
    }
}

impl PartialEq for Version {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Version::Read(left), Version::Read(right)) => Arc::ptr_eq(&left, &right),
            (Version::Write(left), Version::Write(right)) => Arc::ptr_eq(&left, &right),
            _ => false,
        }
    }
}

impl Eq for Version {}

#[repr(C)]
struct ArcInnerLayout<T: ?Sized> {
    strong: AtomicUsize,
    weak: AtomicUsize,
    data: T,
}

struct ArcLayout<T: ?Sized> {
    ptr: NonNull<ArcInnerLayout<T>>,
    phantom: marker<ArcInnerLayout<T>>,
}
