use std::{
    cell::{Cell, UnsafeCell},
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

use crate::addr::PAddrGlobal;

#[derive(Debug)]
pub struct RcRefCellBoxed<T> {
    inner: NonNull<RcRefCellBoxedInner<T>>,
}
#[derive(Debug)]
pub struct RcRefCellBoxedInner<T> {
    ref_count: Cell<u8>,
    borrow_count: Cell<i8>,
    data: UnsafeCell<T>,
}

impl<T> Clone for RcRefCellBoxed<T> {
    fn clone(&self) -> Self {
        self.get_inner()
            .ref_count
            .set(1 + self.get_inner().ref_count.get());
        Self { inner: self.inner }
    }
}

impl<T> Drop for RcRefCellBoxed<T> {
    fn drop(&mut self) {
        let new_count = self.get_inner().ref_count.get() - 1;
        self.get_inner().ref_count.set(new_count);
        if new_count == 0 {
            unsafe {
                core::ptr::drop_in_place(self.inner.as_ptr());
            }
        }
    }
}

impl<T> RcRefCellBoxedInner<T> {
    pub const fn new(value: T) -> Self {
        Self {
            borrow_count: Cell::new(0),
            ref_count: Cell::new(0),
            data: UnsafeCell::new(value),
        }
    }

    pub fn clone(&self) -> RcRefCellBoxed<T> {
        self.ref_count.set(1 + self.ref_count.get());
        RcRefCellBoxed { inner: self.into() }
    }
}

impl<T> RcRefCellBoxed<T> {
    pub fn new_with_inner_location<F: FnOnce(usize, usize) -> PAddrGlobal>(
        data: T,
        space_provider: F,
    ) -> Self {
        let inner = RcRefCellBoxedInner::<T>::new(data);
        let location = space_provider(
            core::mem::size_of_val(&inner),
            core::mem::align_of_val(&inner),
        );
        unsafe {
            let ptr: *mut RcRefCellBoxedInner<T> = location.as_raw_ptr();
            core::ptr::write(ptr, inner);
            Self::new_from_inner(ptr)
        }
    }

    pub unsafe fn new_from_inner(inner: *const RcRefCellBoxedInner<T>) -> Self {
        (*inner).ref_count.set((*inner).ref_count.get() + 1);
        Self {
            inner: NonNull::new(inner as _).unwrap(),
        }
    }

    fn get_inner(&self) -> &RcRefCellBoxedInner<T> {
        unsafe { self.inner.as_ref() }
    }

    pub fn borrow(&self) -> impl Deref<Target = T> + '_ {
        struct Ref<'a, T> {
            data: &'a RcRefCellBoxedInner<T>,
        }

        impl<'a, T> Deref for Ref<'a, T> {
            type Target = T;

            fn deref(&self) -> &Self::Target {
                unsafe { &*self.data.data.get() }
            }
        }

        impl<'a, T> Drop for Ref<'a, T> {
            fn drop(&mut self) {
                self.data.borrow_count.set(self.data.borrow_count.get() - 1);
            }
        }

        let bc = self.get_inner().borrow_count.get();
        if bc < 0 {
            panic!("Already mutably borrowed");
        }
        let new_count = bc + 1;
        self.get_inner().borrow_count.set(new_count);
        Ref {
            data: self.get_inner(),
        }
    }

    pub fn try_borrow_mut(&self) -> Result<impl DerefMut<Target = T> + '_, ()> {
        struct Ref<'a, T> {
            data: &'a RcRefCellBoxedInner<T>,
        }

        impl<'a, T> DerefMut for Ref<'a, T> {
            fn deref_mut(&mut self) -> &mut Self::Target {
                unsafe { &mut *self.data.data.get() }
            }
        }

        impl<'a, T> Deref for Ref<'a, T> {
            type Target = T;

            fn deref(&self) -> &Self::Target {
                unsafe { &*self.data.data.get() }
            }
        }

        impl<'a, T> Drop for Ref<'a, T> {
            fn drop(&mut self) {
                self.data.borrow_count.set(self.data.borrow_count.get() + 1);
            }
        }

        let bc = self.get_inner().borrow_count.get();
        if bc != 0 {
            return Err(());
        }
        let new_count = bc - 1;
        self.get_inner().borrow_count.set(new_count);
        Ok(Ref {
            data: self.get_inner(),
        })
    }

    pub fn borrow_mut(&self) -> impl DerefMut<Target = T> + '_ {
        self.try_borrow_mut().expect("Already borrowed")
    }
}

#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct Boxed<T> {
    inner: NonNull<T>,
}

impl<T> Boxed<T> {
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_rcboxed() {
        let rci1 = Box::new(RcRefCellBoxedInner::new(50u64));
        let rci1_ptr = Box::into_raw(rci1);
        let rc1 = unsafe { RcRefCellBoxed::new_from_inner(rci1_ptr) };
        assert_eq!(1, rc1.get_inner().ref_count.get());
        assert_eq!(0, rc1.get_inner().borrow_count.get());
        {
            let b = rc1.borrow();
            assert_eq!(50, *b);
            assert_eq!(1, rc1.get_inner().ref_count.get());
            assert_eq!(1, rc1.get_inner().borrow_count.get());
        }
        assert_eq!(1, rc1.get_inner().ref_count.get());
        assert_eq!(0, rc1.get_inner().borrow_count.get());
        {
            let b = rc1.borrow_mut();
            assert_eq!(50, *b);
            assert_eq!(1, rc1.get_inner().ref_count.get());
            assert_eq!(-1, rc1.get_inner().borrow_count.get());
        }
        let rc2 = rc1.clone();
        {
            let b = rc1.borrow();
            assert_eq!(50, *b);
            assert_eq!(2, rc1.get_inner().ref_count.get());
            assert_eq!(1, rc2.get_inner().borrow_count.get());
        }
    }
}
