use std::{
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

use crate::addr::PAddrGlobal;

/**
An owned pointer structure similar to Box but
without deallocate support.
*/
#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct Boxed<T> {
    inner: NonNull<T>,
}

impl<T> Boxed<T> {
    /**
    Get the Global paddr of the boxed pointer.
    */
    pub fn paddr_global(&self) -> PAddrGlobal {
        (self.inner.as_ptr() as u64).into()
    }

    pub unsafe fn new(ptr: PAddrGlobal) -> Self {
        // TODO: assert ptr is in correct range.
        Self { inner: ptr.into() }
    }

    pub const unsafe fn new_unchecked(ptr_paddr: u64) -> Self {
        // TODO: assert ptr is in correct range.
        Self {
            inner: NonNull::new_unchecked(ptr_paddr as _),
        }
    }

    fn inner_ptr(&self) -> *mut T {
        self.inner.as_ptr()
    }
}

impl<T> Deref for Boxed<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.inner_ptr() }
    }
}

impl<T> DerefMut for Boxed<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.inner_ptr() }
    }
}

impl<T> Drop for Boxed<T> {
    fn drop(&mut self) {
        unsafe { core::ptr::drop_in_place(self.inner_ptr()) }
    }
}
