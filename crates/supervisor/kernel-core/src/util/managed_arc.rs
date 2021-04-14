//! Managed Arc.
//! This is different in the implementation of the standard library's arc
//! in the sense that memory is considered freed once all strong references
//! go out. This requires weak references to hold space in memory. This is
//! denoted by the [`ManagedWeakNode`] type. Any weak pointer point to this
//! node which then points to the actual contained data - [`ManagedArcInner<T>`].

use crate::addr::PAddrGlobal;
use spin::Mutex;
use std::{
    any::{Any, TypeId},
    fmt::{self, Formatter},
    marker::Unsize,
    mem,
    ops::{CoerceUnsized, Deref, DispatchFromDyn},
    ptr::{self, NonNull},
};

mod rwlock;
mod weak_pool;
pub use rwlock::*;
pub use weak_pool::*;

/// A weak node (entry of a weak pool).
#[derive(Debug, PartialEq)]
struct ManagedWeakNode {
    ptr: NonNull<ManagedArcInner<dyn Any>>,
    prev_weak_node: Option<ManagedWeakAddr>,
    next_weak_node: Option<ManagedWeakAddr>,
}

impl ManagedWeakNode {
    fn get_inner(&self) -> &ManagedArcInner<dyn Any> {
        unsafe { self.ptr.as_ref() }
    }
}

/// A weak address pointing to a [`ManagedWeakNode`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct ManagedWeakAddr {
    weak_node_addr: NonNull<Mutex<Option<ManagedWeakNode>>>,
}

/// Inner of an Arc, containing strong pointers and weak pointers
/// information. Wrap the actual data.
#[repr(C)]
struct ManagedArcInner<T: ?Sized> {
    strong_count: Mutex<usize>,
    /// Pointer to the first weak reference. This also acts as a lock
    /// to the double linked list for the weak pointers.
    first_weak: Mutex<Option<ManagedWeakAddr>>,
    arced_data: T,
}

/// A managed Arc, pointing to a `ManagedArcInner`.
pub struct ManagedArc<T: ?Sized> {
    managed_arc_inner_ptr: NonNull<ManagedArcInner<T>>,
}

impl<T: ?Sized + Unsize<U>, U: ?Sized> CoerceUnsized<ManagedArc<U>> for ManagedArc<T> {}
impl<T: ?Sized + Unsize<U>, U: ?Sized> DispatchFromDyn<ManagedArc<U>> for ManagedArc<T> {}

impl ManagedWeakAddr {
    fn get_object(&self) -> &Mutex<Option<ManagedWeakNode>> {
        unsafe { self.weak_node_addr.as_ref() }
    }

    unsafe fn get_object_mut(&mut self) -> &mut Mutex<Option<ManagedWeakNode>> {
        self.weak_node_addr.as_mut()
    }
}

impl<T: ?Sized> Deref for ManagedArc<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.read_object().arced_data
    }
}

impl Drop for ManagedWeakNode {
    fn drop(&mut self) {
        // Try locking the master
        let mem_object = self.ptr;
        let mut first_weak = unsafe { mem_object.as_ref().first_weak.lock() };

        // Update prev child.
        if let Some(mut prev_weak_addr) = self.prev_weak_node {
            if let Some(prev_weak_node_data) = unsafe { prev_weak_addr.get_object_mut() }.get_mut()
            {
                prev_weak_node_data.next_weak_node = self.next_weak_node;
            } else {
                debug_panic!("Node must exist when there is a WeakNode addr pointed to it.")
            }
        } else {
            // First child
            *first_weak = self.next_weak_node;
        }

        // Update next child.
        if let Some(mut next_weak_node_addr) = self.next_weak_node {
            if let Some(next_weak_node_data) =
                unsafe { next_weak_node_addr.get_object_mut() }.get_mut()
            {
                next_weak_node_data.prev_weak_node = self.prev_weak_node;
            } else {
                debug_panic!("Node must exist when there is a WeakNode addr pointed to it.")
            }
        }
    }
}

impl<T: ?Sized> Drop for ManagedArcInner<T> {
    fn drop(&mut self) {
        let strong_count = self.strong_count.lock();
        assert!(*strong_count == 0);

        let first_weak_data = self.first_weak.lock();

        let mut current_child_addr_option = *first_weak_data;
        while let Some(current_child_addr) = current_child_addr_option {
            let current_child_obj = current_child_addr.get_object();
            let mut current_child_inner = current_child_obj.lock();

            if let Some(current_child_data) = &*current_child_inner {
                current_child_addr_option = current_child_data.next_weak_node;
            } else {
                debug_panic!("Node must exist when there is a WeakNode addr pointed to it.")
            }

            let inner = current_child_inner.take();
            core::mem::forget(inner);
        }
    }
}

impl<T: ?Sized> Drop for ManagedArc<T> {
    fn drop(&mut self) {
        let inner_obj = self.read_object();
        let mut strong_count = inner_obj.strong_count.lock();
        *strong_count -= 1;

        if *strong_count == 0 {
            unsafe {
                core::mem::drop(strong_count);
                core::ptr::drop_in_place(
                    inner_obj as *const ManagedArcInner<T> as *mut ManagedArcInner<T>,
                );
            }
        }
    }
}

impl<T> fmt::Debug for ManagedArc<T> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(
            f,
            "{}({:p})",
            core::any::type_name::<Self>(),
            self.managed_arc_inner_ptr.as_ptr()
        )
    }
}

impl<T: ?Sized> Clone for ManagedArc<T> {
    fn clone(&self) -> Self {
        let inner_obj = self.read_object();
        let mut strong_count = inner_obj.strong_count.lock();
        *strong_count += 1;

        ManagedArc {
            managed_arc_inner_ptr: self.managed_arc_inner_ptr,
        }
    }
}

impl<T: ?Sized> ManagedArc<T> {
    /// Get the ManagedArcInner length.
    pub fn inner_type_length() -> usize
    where
        T: Sized,
    {
        mem::size_of::<ManagedArcInner<T>>()
    }

    /// Get the ManagedArcInner alginment.
    pub fn inner_type_alignment() -> usize
    where
        T: Sized,
    {
        mem::align_of::<ManagedArcInner<T>>()
    }

    /// Create a managed Arc from a physical address.
    /// The pointer should be pointed to an inner ptr.
    pub unsafe fn from_ptr(arc_inner_ptr: PAddrGlobal) -> Result<Self, ()>
    where
        T: Sized,
    {
        let arc_inner_u64: u64 = arc_inner_ptr.into();
        let target_location: NonNull<ManagedArcInner<T>> =
            NonNull::new(arc_inner_u64 as _).unwrap();
        let arc = ManagedArc {
            managed_arc_inner_ptr: target_location,
        };

        let inner_obj = arc.read_object();
        let mut strong_count = inner_obj.strong_count.lock();
        if *strong_count == 0 {
            return Err(());
        }
        *strong_count += 1;

        mem::drop(strong_count);
        Ok(arc)
    }

    fn with_inner(arc_inner: &ManagedArcInner<T>) -> Result<Self, ()> {
        let arc = ManagedArc {
            managed_arc_inner_ptr: arc_inner.into(),
        };

        let inner_obj = arc.read_object();
        let mut strong_count = inner_obj.strong_count.lock();
        if *strong_count == 0 {
            return Err(());
        }
        *strong_count += 1;

        mem::drop(strong_count);
        Ok(arc)
    }

    /// Create a managed Arc using the given data.
    pub unsafe fn new(arc_inner_ptr: PAddrGlobal, data: T) -> Self
    where
        T: Sized,
    {
        let addr_u64: u64 = arc_inner_ptr.into();
        let target_location = NonNull::new(addr_u64 as _).unwrap();
        let arc = ManagedArc {
            managed_arc_inner_ptr: target_location,
        };
        let data_to_write = ManagedArcInner {
            strong_count: Mutex::new(1),
            first_weak: Mutex::new(None),
            arced_data: data,
        };
        ptr::write(target_location.as_ptr(), data_to_write);

        arc
    }

    /// Read the inner object, wrapped in a memory object.
    fn read_object(&self) -> &ManagedArcInner<T> {
        unsafe { self.managed_arc_inner_ptr.as_ref() }
    }

    /// Read the inner object, wrapped in a memory object.
    unsafe fn read_object_mut(&mut self) -> &mut ManagedArcInner<T> {
        self.managed_arc_inner_ptr.as_mut()
    }

    /// Get the strong pointers count.
    pub fn strong_count(&self) -> usize {
        let inner = self.read_object();
        let strong_count = inner.strong_count.lock();
        *strong_count
    }

    pub fn weak_count(&self) -> usize {
        let mut count = 0;
        let inner = self.read_object();
        let first_addr_lock = inner.first_weak.lock();
        let mut cur_addr = *first_addr_lock;
        loop {
            if let Some(next) = cur_addr {
                count += 1;
                let inner_weak = next.get_object();
                let mut next_node_guard = inner_weak.lock();
                if let Some(ref mut next_node) = *next_node_guard {
                    cur_addr = next_node.next_weak_node;
                }
            } else {
                break;
            }
        }
        count
    }
}

pub trait ManagedArcTrait {
    fn get_inner_type() -> TypeId;
}

impl<T: Sized + 'static> ManagedArcTrait for ManagedArc<T> {
    fn get_inner_type() -> TypeId {
        TypeId::of::<T>()
    }
}

pub type ManagedArcAny = ManagedArc<dyn Any>;

impl ManagedArcAny {
    /// Check whether this Arc is of given type.
    /// The target is a `ManagedArc`.
    pub fn is<T: ManagedArcTrait>(&self) -> bool {
        T::get_inner_type() == self.deref().type_id()
    }
}

impl<T: Any> From<ManagedArcAny> for ManagedArc<T> {
    fn from(any: ManagedArcAny) -> Self {
        assert!((*any).type_id() == TypeId::of::<T>(), "Downcast mismatch");
        let arc_inner_ptr = any.managed_arc_inner_ptr;
        let arc_inner_ptr_strong = arc_inner_ptr.cast();
        mem::forget(any);
        ManagedArc {
            managed_arc_inner_ptr: arc_inner_ptr_strong,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::mem::MaybeUninit;

    use super::*;

    #[test]
    fn test_managed_arc() {
        let underlying_value: Box<MaybeUninit<ManagedArcInner<u64>>> =
            Box::new(MaybeUninit::uninit());
        let box_addr = Box::into_raw(underlying_value) as u64;
        let addr = PAddrGlobal::new(box_addr);

        let arc = unsafe { ManagedArc::new(addr, 999u64) };
        assert_eq!(1, arc.strong_count());

        assert_eq!(999, *arc);

        let arc2 = arc.clone();
        assert_eq!(2, arc2.strong_count());
        assert_eq!(999, *arc2);

        mem::drop(arc);
        assert_eq!(1, arc2.strong_count());
    }

    #[test]
    fn test_managed_arc_any() {
        let underlying_value: Box<MaybeUninit<ManagedArcInner<u64>>> =
            Box::new(MaybeUninit::uninit());
        let box_addr = Box::into_raw(underlying_value) as u64;
        let addr = PAddrGlobal::new(box_addr);

        let arc: ManagedArc<u64> = unsafe { ManagedArc::new(addr, 999u64) };
        let arc2 = arc.clone();
        let any: ManagedArcAny = arc;
        assert!(!any.is::<ManagedArc<u8>>());
        assert!(any.is::<ManagedArc<u64>>());

        assert_eq!(
            2,
            arc2.strong_count(),
            "The original arc shouldn't be dropped."
        );

        let strong: ManagedArc<u64> = any.into();
        assert_eq!(999, *strong);

        assert_eq!(2, strong.strong_count());
    }
}
