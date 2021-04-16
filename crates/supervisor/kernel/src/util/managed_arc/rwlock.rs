use crate::util::managed_arc::ManagedArc;
use core::ops::{Deref, DerefMut};
use spin::{RwLock, RwLockReadGuard, RwLockWriteGuard};

/// A read guard for ManagedArc.
pub struct ManagedArcRwLockReadGuard<'a, T: 'a> {
    lock: RwLockReadGuard<'a, T>,
}

impl<'a, T: 'a> Deref for ManagedArcRwLockReadGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.lock.deref()
    }
}

/// A write guard for ManagedArc.
pub struct ManagedArcRwLockWriteGuard<'a, T: 'a> {
    lock: RwLockWriteGuard<'a, T>,
}

impl<'a, T: 'a> Deref for ManagedArcRwLockWriteGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.lock.deref()
    }
}

impl<'a, T: 'a> DerefMut for ManagedArcRwLockWriteGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.lock.deref_mut()
    }
}

impl<U: core::fmt::Debug> ManagedArc<RwLock<U>> {
    /// Read the value from the ManagedArc. Returns the guard.
    pub fn read(&self) -> ManagedArcRwLockReadGuard<U> {
        let inner_obj = self.read_object();
        ManagedArcRwLockReadGuard {
            lock: inner_obj.arced_data.read(),
        }
    }

    /// Write to the ManagedArc. Returns the guard.
    pub fn write(&self) -> ManagedArcRwLockWriteGuard<U> {
        let inner_obj = self.read_object();
        ManagedArcRwLockWriteGuard {
            lock: inner_obj.arced_data.write(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use super::*;
    use std::mem::MaybeUninit;

    #[test]
    fn test_rw_lock() {
        let underlying_value: Box<MaybeUninit<ManagedArcInner<RwLock<u64>>>> =
            Box::new(MaybeUninit::uninit());
        let box_addr = Box::into_raw(underlying_value) as u64;
        let addr = PAddrGlobal::new(box_addr);

        let arc = unsafe { ManagedArc::new(addr, RwLock::new(5u64)) };
        assert_eq!(5, *arc.read());
        *arc.write() = 6;
        assert_eq!(6, *arc.read());
    }
}
