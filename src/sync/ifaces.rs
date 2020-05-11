pub unsafe trait LockIface {
    fn lock(&self);

    fn try_lock(&self) -> bool;

    fn is_locked(&self) -> bool;

    fn unlock(&self);

    fn try_unlock(&self) -> bool;
}

pub unsafe trait RwLockIface {
    fn try_lock_read(&mut self) -> bool;

    fn try_release_read(&mut self) -> bool;

    fn try_lock_write(&mut self) -> bool;

    fn try_release_write(&mut self) -> bool;
}
