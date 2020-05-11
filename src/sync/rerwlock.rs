use super::{
    ifaces::RwLockIface,
    ttas::{TTas, TTasGuard},
};
use std::cell::UnsafeCell;
use std::{
    fmt,
    time::{Duration, Instant},
};
use std::{
    marker::PhantomData as marker,
    ops::{Deref, DerefMut},
};
use std::{thread, thread::ThreadId};

const READ_OPTIMIZED_ALLOC: usize = 50_usize;

struct ThreadRef {
    id: ThreadId,
    count: usize,
}

impl ThreadRef {
    #[inline]
    pub fn new(count: usize) -> Self {
        Self {
            id: thread::current().id(),
            count,
        }
    }

    #[inline]
    pub fn is_current(&self) -> bool {
        thread::current().id() == self.id
    }

    #[inline]
    pub fn try_inc(&mut self) -> bool {
        if self.is_current() {
            self.count = match self.count.checked_add(1) {
                Some(x) => x,
                _ => return false,
            };
            true
        } else {
            false
        }
    }

    #[inline]
    pub fn try_dec(&mut self) -> bool {
        if self.is_current() {
            self.count = match self.count.checked_sub(1) {
                Some(x) => x,
                _ => return false,
            };
            true
        } else {
            false
        }
    }

    #[inline]
    pub fn is_positive(&self) -> bool {
        self.count > 0
    }
}

impl fmt::Debug for ThreadRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ThreadRef")
            .field("id", &self.id)
            .field("count", &self.count)
            .finish()
    }
}

struct Container {
    writer: Option<ThreadRef>,
    readers: Vec<ThreadRef>,
}

impl Container {
    pub fn new() -> Self {
        Self {
            writer: None,
            readers: Vec::with_capacity(READ_OPTIMIZED_ALLOC),
        }
    }

    pub fn readers_from_single_thread(&self) -> (bool, Option<&ThreadRef>) {
        let mut reader = None;
        for counter in self.readers.iter() {
            if counter.is_positive() {
                match reader {
                    Some(_) => return (false, None),
                    None => reader = Some(counter),
                }
            }
        }
        (true, reader)
    }

    fn readers_for_current_thread(&mut self) -> &mut ThreadRef {
        match self.readers.iter().position(|c| c.is_current()) {
            Some(index) => &mut self.readers[index],
            None => {
                self.readers.push(ThreadRef::new(0_usize));
                self.readers
                    .last_mut()
                    .expect("Last element was just added right before!")
            }
        }
    }

    fn writer_from_current_thread(&mut self) -> bool {
        self.writer.as_ref().map_or(false, |ow| ow.is_current())
    }
}

// Write Guard

pub struct ReentrantWriteGuard<'a, T: ?Sized>
where
    ReentrantRwLock<T>: 'a,
{
    lock: &'a ReentrantRwLock<T>,
    marker: marker<&'a mut T>,
}

impl<'a, T: ?Sized> Deref for ReentrantWriteGuard<'a, T>
where
    ReentrantRwLock<T>: 'a,
{
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T: ?Sized> DerefMut for ReentrantWriteGuard<'a, T>
where
    ReentrantRwLock<T>: 'a,
{
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<'a, T: ?Sized> Drop for ReentrantWriteGuard<'a, T> {
    fn drop(&mut self) {
        let mut c = self.lock.get_container().unwrap();
        c.try_release_write();
        if thread::panicking() {
            // TODO: Drop all the guards on poisoned data.
            // c.try_release_write();
            c.try_release_read();
        }
    }
}

impl<'a, T> fmt::Debug for ReentrantWriteGuard<'a, T>
where
    T: fmt::Debug + ?Sized + 'a,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<'a, T> fmt::Display for ReentrantWriteGuard<'a, T>
where
    T: fmt::Display + ?Sized + 'a,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

// Read Guard

pub struct ReentrantReadGuard<'a, T: ?Sized>
where
    ReentrantRwLock<T>: 'a,
{
    lock: &'a ReentrantRwLock<T>,
    marker: marker<&'a T>,
}

impl<'a, T: ?Sized> Deref for ReentrantReadGuard<'a, T>
where
    ReentrantRwLock<T>: 'a,
{
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T: ?Sized> Drop for ReentrantReadGuard<'a, T> {
    fn drop(&mut self) {
        self.lock.get_container().unwrap().try_release_read();
    }
}

impl<'a, T> fmt::Debug for ReentrantReadGuard<'a, T>
where
    T: fmt::Debug + ?Sized + 'a,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<'a, T> fmt::Display for ReentrantReadGuard<'a, T>
where
    T: fmt::Display + ?Sized + 'a,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

///
/// Lock-free Reentrant RW Lock implementation.
pub struct ReentrantRwLock<T>
where
    T: ?Sized,
{
    container: TTas<Container>,
    data: UnsafeCell<T>,
}

unsafe impl<T: ?Sized + Send> Send for ReentrantRwLock<T> {}
unsafe impl<T: ?Sized + Send> Sync for ReentrantRwLock<T> {}

impl<T> ReentrantRwLock<T>
where
    T: ?Sized,
{
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        unsafe { &mut *self.data.get() }
    }

    #[inline]
    fn get_container(&self) -> Option<TTasGuard<Container>> {
        self.container.try_lock()
    }
}

impl<T> ReentrantRwLock<T> {
    pub fn new(data: T) -> Self {
        Self {
            container: TTas::new(Container::new()),
            data: UnsafeCell::new(data),
        }
    }

    #[inline]
    pub fn into_inner(self) -> T {
        self.data.into_inner()
    }

    // Exposed methods

    #[inline]
    pub fn read(&self) -> ReentrantReadGuard<'_, T> {
        loop {
            match self.try_read() {
                Some(guard) => return guard,
                None => thread::yield_now(),
            }
        }
    }

    #[inline]
    pub fn try_read(&self) -> Option<ReentrantReadGuard<'_, T>> {
        let cont = self.get_container();
        match cont {
            Some(mut c) => {
                if c.try_lock_read() {
                    Some(ReentrantReadGuard { lock: self, marker })
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    #[inline]
    pub fn write(&self) -> ReentrantWriteGuard<'_, T> {
        loop {
            match self.try_write() {
                Some(guard) => return guard,
                None => thread::yield_now(),
            }
        }
    }

    #[inline]
    pub fn try_write(&self) -> Option<ReentrantWriteGuard<'_, T>> {
        let cont = self.get_container();
        match cont {
            Some(mut c) => {
                if c.try_lock_write() {
                    Some(ReentrantWriteGuard { lock: self, marker })
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    #[inline]
    pub fn is_locked(&self) -> bool {
        self.try_write().is_none()
    }

    #[inline]
    pub fn try_write_lock_for(&self, timeout: Duration) -> Option<ReentrantWriteGuard<'_, T>> {
        let deadline = Instant::now()
            .checked_add(timeout)
            .expect("Deadline can't fit in");
        loop {
            if Instant::now() < deadline {
                match self.try_write() {
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
    pub fn is_writer_held_by_current(&self) -> bool {
        loop {
            if let Some(mut cont) = self.get_container() {
                break cont.writer_from_current_thread();
            } else {
                thread::yield_now();
            }
        }
    }
}

unsafe impl RwLockIface for Container {
    fn try_lock_read(&mut self) -> bool {
        if let Some(holder) = &mut self.writer {
            if !holder.is_current() {
                return false;
            }
        }
        self.readers_for_current_thread().try_inc()
    }

    fn try_release_read(&mut self) -> bool {
        self.readers_for_current_thread().try_dec()
    }

    fn try_lock_write(&mut self) -> bool {
        if let Some(holder) = &mut self.writer {
            return holder.try_inc();
        }

        match self.readers_from_single_thread() {
            (true, Some(holder)) => {
                if !holder.is_current() {
                    return false;
                }
            }
            // (true, None) => {}
            (false, _) => return false,
            _ => {}
        }
        self.writer = Some(ThreadRef::new(1_usize));

        true
    }

    fn try_release_write(&mut self) -> bool {
        match &mut self.writer {
            Some(holder) => holder.try_dec(),
            None => false,
        }
    }
}

#[cfg(test)]
mod reentrant_lock_tests {
    use super::*;

    #[test]
    fn rwlock_create_and_reacquire_write_lock() {
        let rew = ReentrantRwLock::new(144);
        let data = rew.try_read();

        assert!(data.is_some());

        assert!(rew.try_read().is_some());
        assert!(rew.try_read().is_some());

        core::mem::drop(data);

        assert!(rew.try_write().is_some());
        assert!(rew.try_read().is_some());
    }

    #[test]
    fn rwlock_create_and_reacquire_read_lock() {
        let rew = ReentrantRwLock::new(144);
        let data = rew.try_read();

        assert!(data.is_some());

        assert!(rew.try_read().is_some());
        assert!(rew.try_read().is_some());

        core::mem::drop(data);

        assert!(rew.try_read().is_some());
        assert!(rew.try_write().is_some());
    }

    #[test]
    fn rwlock_reacquire_without_drop() {
        let rew = ReentrantRwLock::new(144);
        let datar = rew.read();
        assert_eq!(*datar, 144);

        assert!(rew.try_read().is_some());
        assert!(rew.try_read().is_some());
        assert!(rew.try_write().is_some());

        // Write data while holding read guard
        let mut dataw = rew.write();
        *dataw += 288;

        // Read after write guard
        let datar2 = rew.read();
        assert_eq!(*datar2, 432);
    }
}
