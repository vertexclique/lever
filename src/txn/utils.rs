use crate::txn::vars::TVar;
use crate::txn::version::*;
use std::any::Any;
use std::sync::Arc;

pub(crate) fn convert_ref<R: Any + Clone + Send + Sync>(from: Var) -> R {
    (&*from as &dyn Any).downcast_ref::<R>().unwrap().clone()
}

// TODO: Nightly stuff, polish up a bit with feature gates.
// pub fn print_type_of<T>(_: &T) {
//     println!("{}", unsafe { std::intrinsics::type_name::<T>() });
// }

pub(crate) fn direct_convert_ref<R: Any + Clone + Send + Sync>(from: &Var) -> R {
    (&*from as &dyn Any).downcast_ref::<R>().unwrap().clone()
}

pub(crate) fn downcast<R: 'static + Clone>(var: Arc<dyn Any>) -> R {
    match var.downcast_ref::<R>() {
        Some(s) => s.clone(),
        None => unreachable!("Requested wrong type for Var"),
    }
}

pub(crate) fn version_to_tvar<T: Any + Clone + Send + Sync>(ver: &Version) -> TVar<T> {
    let x: *const dyn Any = Arc::into_raw(ver.read());
    let xptr: *const TVar<T> = x as *const TVar<T>;
    let k: Arc<TVar<T>> = unsafe { Arc::from_raw(xptr) };
    let k: TVar<T> = downcast(k);
    k
}

pub(crate) fn version_to_dest<T: Any + Clone + Send + Sync>(ver: &Version) -> T {
    let x: *const dyn Any = Arc::into_raw(ver.read());
    let xptr: *const T = x as *const T;
    let k: Arc<T> = unsafe { Arc::from_raw(xptr) };
    let k: T = downcast(k);
    k
}
