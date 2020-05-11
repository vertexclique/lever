use super::ifaces::LockIface;
use std::fmt;
use std::{
    cell::UnsafeCell,
    sync::atomic::{spin_loop_hint, AtomicBool, Ordering},
};
use std::{
    marker::PhantomData as marker,
    ops::{Deref, DerefMut},
    thread::ThreadId,
    time::{Duration, Instant},
};

pub struct TTasGuard<'a, T: ?Sized> {
    mutex: &'a TTas<T>,
    marker: marker<&'a mut T>,
}

impl<'a, T: ?Sized + 'a> Deref for TTasGuard<'a, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.mutex.data.get() }
    }
}

impl<'a, T: ?Sized + 'a> DerefMut for TTasGuard<'a, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.mutex.data.get() }
    }
}

impl<'a, T: ?Sized + 'a> Drop for TTasGuard<'a, T> {
    #[inline]
    fn drop(&mut self) {
        self.mutex.unlock();
    }
}

impl<'a, T: fmt::Debug + ?Sized + 'a> fmt::Debug for TTasGuard<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<'a, T: fmt::Display + ?Sized + 'a> fmt::Display for TTasGuard<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

pub struct TTas<T>
where
    T: ?Sized,
{
    tid: ThreadId,
    acquired: AtomicBool,
    data: UnsafeCell<T>,
}

impl<T> TTas<T> {
    #[inline]
    pub fn new(data: T) -> Self {
        Self {
            tid: std::thread::current().id(),
            acquired: AtomicBool::default(),
            data: UnsafeCell::new(data),
        }
    }

    #[inline]
    pub fn into_inner(self) -> T {
        self.data.into_inner()
    }

    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        unsafe { &mut *self.data.get() }
    }

    #[inline]
    unsafe fn guard(&self) -> TTasGuard<'_, T> {
        TTasGuard {
            mutex: self,
            marker,
        }
    }

    #[inline]
    pub fn lock(&self) -> TTasGuard<'_, T> {
        // TODO: dispatch directly
        <Self as LockIface>::lock(self);
        // SAFETY: The lock is held, as required.
        unsafe { self.guard() }
    }

    #[inline]
    pub fn try_lock(&self) -> Option<TTasGuard<'_, T>> {
        if <Self as LockIface>::try_lock(self) {
            // SAFETY: The lock is held, as required.
            Some(unsafe { self.guard() })
        } else {
            None
        }
    }

    #[inline]
    pub unsafe fn force_unlock(&self) {
        <Self as LockIface>::unlock(&self);
    }

    #[inline]
    pub fn try_write_lock_for(&self, timeout: Duration) -> Option<TTasGuard<'_, T>> {
        let deadline = Instant::now()
            .checked_add(timeout)
            .expect("Deadline can't fit in");
        loop {
            if Instant::now() < deadline {
                match self.try_lock() {
                    Some(guard) => {
                        break Some(guard);
                    }
                    _ => {
                        std::thread::sleep(timeout / 10);
                        std::thread::yield_now()
                    }
                };
            } else {
                break None;
            }
        }
    }

    #[inline]
    pub fn is_current(&self) -> bool {
        std::thread::current().id() == self.tid
    }
}

unsafe impl<T: ?Sized + Send> Send for TTas<T> {}
unsafe impl<T: ?Sized + Send> Sync for TTas<T> {}

unsafe impl<T> LockIface for TTas<T>
where
    T: ?Sized,
{
    #[inline]
    fn lock(&self) {
        'lock: loop {
            while let Some(true) = Some(self.acquired.load(Ordering::SeqCst)) {
                spin_loop_hint();
            }
            if !self.acquired.swap(true, Ordering::SeqCst) {
                break 'lock;
            }
        }
    }

    #[inline]
    fn try_lock(&self) -> bool {
        self.acquired
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }

    #[inline]
    fn is_locked(&self) -> bool {
        self.acquired.load(Ordering::Acquire)
    }

    #[inline]
    fn unlock(&self) {
        self.acquired.store(false, Ordering::Release);
    }

    #[inline]
    fn try_unlock(&self) -> bool {
        self.acquired
            .compare_exchange_weak(true, false, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }
}

#[cfg(test)]
mod test_ttas {
    use super::*;

    #[test]
    fn ttas_create_and_lock() {
        let ttas = TTas::new(42);
        let data = ttas.try_lock();
        assert!(data.is_some());
        assert_eq!(*data.unwrap(), 42);
    }

    #[test]
    fn mutual_exclusion() {
        let ttas = TTas::new(1);
        let data = ttas.try_lock();

        assert!(data.is_some());

        assert!(ttas.try_lock().is_none());
        assert!(ttas.try_lock().is_none());

        core::mem::drop(data);

        assert!(ttas.try_lock().is_some());
    }

    #[test]
    fn three_locks() {
        let ttas1 = TTas::new(1);
        let ttas2 = TTas::new(2);
        let ttas3 = TTas::new(3);

        let data1 = ttas1.try_lock();
        let data2 = ttas2.try_lock();
        let data3 = ttas3.try_lock();

        assert!(data1.is_some());
        assert!(data2.is_some());
        assert!(data3.is_some());

        assert!(ttas1.try_lock().is_none());
        assert!(ttas1.try_lock().is_none());
        assert!(ttas2.try_lock().is_none());
        assert!(ttas2.try_lock().is_none());
        assert!(ttas3.try_lock().is_none());
        assert!(ttas3.try_lock().is_none());

        core::mem::drop(data3);

        assert!(ttas3.try_lock().is_some());
    }
}
