//! Utility to read physical memory as objects.
use std::{
    fmt,
    marker::Unsize,
    ops::{CoerceUnsized, Deref, DerefMut},
    ptr::NonNull,
};

use crate::addr::PAddr;

/// Represent a memory object, that converts a physical address to an
/// accessible object.
///
/// # Safety
///
/// If you are going to wrap MemoryObject in any other struct, you
/// must make should that it is dropped last.
///
/// `UniqueReadGuard` and `UniqueWriteGuard` requires T must be Sized.
#[derive(Debug)]
pub struct MemoryObject<T: ?Sized> {
    paddr: PAddr,
    mapping_start_index: usize,
    mapping_size: usize,
    pointer: NonNull<T>,
}

/// `ObjectGuard` pointers are not `Send` because the data they reference may be aliased.
impl<T: ?Sized> !Send for MemoryObject<T> {}

/// `ObjectGuard` pointers are not `Sync` because the data they reference may be aliased.
impl<T: ?Sized> !Sync for MemoryObject<T> {}

impl<T: ?Sized, U: ?Sized> CoerceUnsized<MemoryObject<U>> for MemoryObject<T> where T: Unsize<U> {}

impl<T: ?Sized> MemoryObject<T> {
    /// Physical address of the memory object.
    pub fn paddr(&self) -> PAddr {
        self.paddr
    }

    /// Create a new memory object. For tests, paddr is the target vaddr.
    ///
    /// # Safety
    ///
    /// PAddr must be a non-zero pointer.
    #[cfg(test)]
    pub unsafe fn new(paddr: PAddr) -> Self
    where
        T: Sized,
    {
        Self {
            paddr: paddr.into(),
            mapping_start_index: 0,
            mapping_size: 0,
            pointer: paddr.into(),
        }
    }

    #[cfg(not(test))]
    pub unsafe fn new(_paddr: PAddr) -> Self
    where
        T: Sized,
    {
        todo!()
    }

    pub fn as_ptr(&self) -> *mut T {
        self.pointer.as_ptr()
    }

    pub unsafe fn as_ref(&self) -> &T {
        &*self.as_ptr()
    }

    pub unsafe fn as_mut(&mut self) -> &mut T {
        &mut *self.as_ptr()
    }
}

impl<T: ?Sized> fmt::Pointer for MemoryObject<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Pointer::fmt(&self.pointer.as_ptr(), f)
    }
}

/// Read guard using a memory object.
pub struct UniqueReadGuard<T> {
    object: MemoryObject<T>,
}

/// Write guard using a memory object.
pub struct UniqueWriteGuard<T> {
    object: MemoryObject<T>,
}

// Implementation for UniqueReadGuard

impl<T> UniqueReadGuard<T> {
    /// Create a new read guard from a memory object.
    pub const unsafe fn new(object: MemoryObject<T>) -> Self {
        UniqueReadGuard::<T> { object }
    }
}

unsafe impl<T> Send for UniqueReadGuard<T> {}
unsafe impl<T> Sync for UniqueReadGuard<T> {}

impl<T> Deref for UniqueReadGuard<T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { self.object.as_ref() }
    }
}

// Implementation for UniqueWriteGuard

impl<T> UniqueWriteGuard<T> {
    /// Create a new write guard using a memory object.
    pub const unsafe fn new(object: MemoryObject<T>) -> Self {
        UniqueWriteGuard::<T> { object }
    }
}

unsafe impl<T> Send for UniqueWriteGuard<T> {}
unsafe impl<T> Sync for UniqueWriteGuard<T> {}

impl<T> Deref for UniqueWriteGuard<T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { self.object.as_ref() }
    }
}

impl<T> DerefMut for UniqueWriteGuard<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { self.object.as_mut() }
    }
}
