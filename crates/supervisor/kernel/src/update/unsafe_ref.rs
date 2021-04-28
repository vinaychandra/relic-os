use core::borrow::Borrow;
use core::fmt;
use core::ops::Deref;
use core::ptr::NonNull;

/// Unchecked shared pointer
///
/// This type acts like a `Rc` or `Arc` except that no reference count is
/// maintained. Instead, the user is responsible for freeing the managed object
/// once it is no longer in use.
///
/// You must guarantee that an object managed by an `UnsafeRef` is not
/// moved, dropped or accessed through a mutable reference as long as at least
/// one `UnsafeRef` is pointing to it.
pub struct UnsafeRef<T: ?Sized> {
    ptr: NonNull<T>,
}

impl<T: ?Sized> UnsafeRef<T> {
    /// Creates an `UnsafeRef` from a raw pointer
    ///
    /// # Safety
    ///
    /// You must ensure that the `UnsafeRef` guarantees are upheld.
    #[inline]
    pub unsafe fn from_raw(val: *const T) -> UnsafeRef<T> {
        UnsafeRef {
            ptr: NonNull::new_unchecked(val as *mut _),
        }
    }

    /// Converts an `UnsafeRef` into a raw pointer
    #[inline]
    pub fn into_raw(ptr: Self) -> *mut T {
        ptr.ptr.as_ptr()
    }
}

impl<T: ?Sized> Clone for UnsafeRef<T> {
    #[inline]
    fn clone(&self) -> UnsafeRef<T> {
        UnsafeRef { ptr: self.ptr }
    }
}

impl<T: ?Sized> Deref for UnsafeRef<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        self.as_ref()
    }
}

impl<T: ?Sized> AsRef<T> for UnsafeRef<T> {
    #[inline]
    fn as_ref(&self) -> &T {
        unsafe { self.ptr.as_ref() }
    }
}

impl<T: ?Sized> Borrow<T> for UnsafeRef<T> {
    #[inline]
    fn borrow(&self) -> &T {
        self.as_ref()
    }
}

// impl<T: fmt::Debug + ?Sized> fmt::Debug for UnsafeRef<T> {
//     #[inline]
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         fmt::Debug::fmt(self.as_ref(), f)
//     }
// }

impl<T: fmt::Debug + ?Sized> fmt::Debug for UnsafeRef<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("UnsafeCell {:p}", self.ptr))
    }
}

unsafe impl<T: ?Sized + Send> Send for UnsafeRef<T> {}

unsafe impl<T: ?Sized + Sync> Sync for UnsafeRef<T> {}

impl<T> From<&T> for UnsafeRef<T> {
    fn from(val: &T) -> Self {
        unsafe { UnsafeRef::from_raw(val) }
    }
}
