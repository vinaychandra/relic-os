//! Managed Arc
//! This is different in the implementation of the standard library's arc
//! in the sense that memory is considered freed once all strong references
//! go out. This requires weak references to hold space in memory. This is
//! denoted by the [`ManagedWeakNode`] type. Any weak pointer point to this
//! node which then points to the actual contained data - [`ManagedArcInner<T>`].

use crate::{addr::PAddr, util::memory_object::MemoryObject};
use spin::Mutex;
use std::{
    any::{Any, TypeId},
    fmt::{self, Formatter},
    marker::PhantomData,
    mem, ptr,
};

mod rwlock;
mod weak_pool;
pub use rwlock::*;
pub use weak_pool::*;

/// A weak node (entry of a weak pool).
#[derive(Debug)]
struct ManagedWeakNode {
    managed_arc_inner_ptr: PAddr,
    managed_arc_type_id: TypeId,
    prev_weak_node: Option<ManagedWeakAddr>,
    next_weak_node: Option<ManagedWeakAddr>,
}

/// A weak address.
#[derive(Copy, Clone, Debug)]
struct ManagedWeakAddr {
    weak_node_addr: PAddr,
}

impl ManagedWeakAddr {
    fn get_object(&self) -> MemoryObject<Mutex<Option<ManagedWeakNode>>> {
        unsafe { MemoryObject::new(self.weak_node_addr) }
    }
}

/// Inner of an Arc, containing strong pointers and weak pointers
/// information. Wrap the actual data.
struct ManagedArcInner<T> {
    strong_count: Mutex<usize>,
    first_weak: Option<ManagedWeakAddr>,
    arced_data: T,
}

impl<T> Drop for ManagedArcInner<T> {
    fn drop(&mut self) {
        let lead = self.strong_count.lock();
        assert!(*lead == 0);

        // TODO drop all weak pointers
        panic!("Trying to drop an inner!");
    }
}

/// A managed Arc, pointing to a `ManagedArcInner`.
pub struct ManagedArc<T> {
    managed_arc_inner_ptr: PAddr,
    _marker: PhantomData<T>,
}

impl<T> fmt::Debug for ManagedArc<T> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(
            f,
            "{}(0x{:x})",
            core::any::type_name::<Self>(),
            self.managed_arc_inner_ptr
        )
    }
}

impl<T> Drop for ManagedArc<T> {
    fn drop(&mut self) {
        let mut inner_obj = self.read_object();
        let inner_obj_mut = unsafe { inner_obj.as_mut() };
        let mut strong_count = inner_obj_mut.strong_count.lock();
        *strong_count -= 1;
    }
}

impl<T> Clone for ManagedArc<T> {
    fn clone(&self) -> Self {
        let mut inner_obj = self.read_object();
        let inner_obj_mut = unsafe { inner_obj.as_mut() };
        let mut strong_count = inner_obj_mut.strong_count.lock();
        *strong_count += 1;

        ManagedArc {
            managed_arc_inner_ptr: self.managed_arc_inner_ptr,
            _marker: PhantomData,
        }
    }
}

impl<T> ManagedArc<T> {
    /// Get the ManagedArcInner length.
    pub fn inner_type_length() -> usize {
        mem::size_of::<ManagedArcInner<T>>()
    }

    /// Get the ManagedArcInner alginment.
    pub fn inner_type_alignment() -> usize {
        mem::align_of::<ManagedArcInner<T>>()
    }

    /// Create a managed Arc from a physical address.
    pub unsafe fn from_ptr(arc_inner_ptr: PAddr) -> Self {
        let arc = ManagedArc {
            managed_arc_inner_ptr: arc_inner_ptr,
            _marker: PhantomData,
        };

        let inner_obj = arc.read_object();
        let inner_ref = inner_obj.as_ref();
        let mut strong_count = inner_ref.strong_count.lock();
        *strong_count += 1;

        arc
    }

    /// Create a managed Arc using the given data.
    pub unsafe fn new(arc_inner_ptr: PAddr, data: T) -> Self {
        let arc = ManagedArc {
            managed_arc_inner_ptr: arc_inner_ptr,
            _marker: PhantomData,
        };
        let mut inner = arc.read_object();
        let data_to_write = ManagedArcInner {
            strong_count: Mutex::new(1),
            first_weak: None,
            arced_data: data,
        };
        ptr::write(inner.as_mut(), data_to_write);

        arc
    }

    /// Read the inner object, wrapped in a memory object.
    fn read_object(&self) -> MemoryObject<ManagedArcInner<T>> {
        unsafe { MemoryObject::<ManagedArcInner<T>>::new(self.managed_arc_inner_ptr) }
    }

    /// Get the strong pointers count.
    pub fn strong_count(&self) -> usize {
        let inner = self.read_object();
        let strong_count = unsafe { inner.as_ref().strong_count.lock() };
        *strong_count
    }

    pub fn weak_count(&self) -> usize {
        let mut count = 0;
        let obj = self.read_object();
        let first_obj = unsafe { obj.as_ref() };
        let mut cur_addr = first_obj.first_weak;
        loop {
            if let Some(next) = cur_addr {
                count += 1;
                let obj = next.get_object();
                let mut next_node_guard = unsafe { obj.as_ref().lock() };
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

/// Like `ManagedArc<T>`, but use `TypeId` to represent its type.
#[derive(Debug)]
pub struct ManagedArcAny {
    managed_arc_inner_ptr: PAddr,
    type_id: TypeId,
}

impl ManagedArcAny {
    /// Check whether this Arc is of given type.
    pub fn is<T: Any>(&self) -> bool
    where
        ManagedArc<T>: Any,
    {
        self.type_id == TypeId::of::<T>()
    }
}

impl<T: Any> From<ManagedArcAny> for ManagedArc<T> {
    fn from(any: ManagedArcAny) -> Self {
        assert!(any.type_id == TypeId::of::<ManagedArc<T>>());
        let arc_inner_ptr = any.managed_arc_inner_ptr;
        mem::forget(any);
        ManagedArc {
            managed_arc_inner_ptr: arc_inner_ptr,
            _marker: PhantomData,
        }
    }
}

impl<T: Any> Into<ManagedArcAny> for ManagedArc<T> {
    fn into(self) -> ManagedArcAny {
        let ptr = self.managed_arc_inner_ptr;
        mem::forget(self);
        ManagedArcAny {
            managed_arc_inner_ptr: ptr,
            type_id: TypeId::of::<ManagedArc<T>>(),
        }
    }
}

impl Drop for ManagedArcAny {
    fn drop(&mut self) {
        panic!("Error: Trying to drop a ManagedArcAny.");
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
        let addr = PAddr::new(box_addr);

        let arc = unsafe { ManagedArc::new(addr, 999u64) };
        assert_eq!(1, arc.strong_count());

        let inner = arc.read_object();
        let obj = unsafe { inner.as_ref() };
        assert_eq!(999, obj.arced_data);

        let arc2 = arc.clone();
        assert_eq!(2, arc2.strong_count());

        let inner = arc2.read_object();
        let obj = unsafe { inner.as_ref() };
        assert_eq!(999, obj.arced_data);

        mem::drop(arc);
        assert_eq!(1, arc2.strong_count());
    }

    #[test]
    fn test_managed_arc_any() {
        let underlying_value: Box<MaybeUninit<ManagedArcInner<u64>>> =
            Box::new(MaybeUninit::uninit());
        let box_addr = Box::into_raw(underlying_value) as u64;
        let addr = PAddr::new(box_addr);

        let arc = unsafe { ManagedArc::new(addr, 999u64) };
        let arc2 = arc.clone();
        let any: ManagedArcAny = arc.into();
        assert!(!any.is::<ManagedArc<u8>>());
        assert!(any.is::<ManagedArc<u64>>());

        assert_eq!(
            2,
            arc2.strong_count(),
            "The original arc shouldn't be dropped."
        );

        let strong: ManagedArc<u64> = any.into();
        let inner = strong.read_object();
        let obj = unsafe { inner.as_ref() };
        assert_eq!(999, obj.arced_data);

        assert_eq!(2, strong.strong_count());
    }
}
