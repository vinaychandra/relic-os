use crate::{
    addr::PAddr,
    util::{
        managed_arc::{
            ManagedArc, ManagedArcAny, ManagedArcInner, ManagedWeakAddr, ManagedWeakNode,
        },
        memory_object::MemoryObject,
    },
};
use spin::Mutex;
use std::{
    any::{Any, TypeId},
    marker::PhantomData,
    ops::Deref,
    ptr,
};

/// A pool of managed weak nodes.
#[derive(Debug)]
pub struct ManagedWeakPool<const SIZE: usize> {
    pool_items: [Mutex<Option<ManagedWeakNode>>; SIZE],
    this_pool_location: PAddr,
}

/// Managed Arc for weak pool of size 1.
pub type ManagedWeakPool1Arc = ManagedArc<ManagedWeakPool<1>>;
/// Managed Arc for weak pool of size 3.
pub type ManagedWeakPool3Arc = ManagedArc<ManagedWeakPool<3>>;
/// Managed Arc for weak pool of size 256.
pub type ManagedWeakPool256Arc = ManagedArc<ManagedWeakPool<256>>;

/// Guard for managed weak pool.
struct ManagedWeakPoolGuard<'a, T: 'a> {
    _phantom: PhantomData<&'a ()>,
    arc_inner_object: MemoryObject<ManagedArcInner<T>>,
}

impl<'a, T: 'a> Deref for ManagedWeakPoolGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &self.arc_inner_object.as_ref().arced_data }
    }
}

impl<const SIZE: usize> ManagedArc<ManagedWeakPool<SIZE>> {
    /// Create a managed weak pool in the given physical address.
    pub unsafe fn create(pool_addr: PAddr) -> Self {
        let arc = ManagedArc::new(pool_addr, core::mem::MaybeUninit::uninit().assume_init());
        let mut inner_obj = arc.read_object();
        let inner: &mut ManagedArcInner<ManagedWeakPool<SIZE>> = inner_obj.as_mut();

        // Determine the offset of the object within the inner
        let offset = (&inner.arced_data as *const _ as usize) - (inner as *const _ as usize);

        ptr::write(&mut inner.arced_data.this_pool_location, pool_addr + offset);
        for element in inner.arced_data.pool_items.iter_mut() {
            ptr::write(element, Mutex::new(None));
        }

        arc
    }

    /// Returns the guard to read the pool.
    pub fn read(&self) -> impl Deref<Target = ManagedWeakPool<SIZE>> + '_ {
        ManagedWeakPoolGuard {
            _phantom: PhantomData,
            arc_inner_object: self.read_object(),
        }
    }
}

impl<const SIZE: usize> ManagedWeakPool<SIZE> {
    /// Create a new strong pointer if `index` points to a
    /// non-none weak pointer in the weak pool.
    /// This requires a function for creation because a ManagedArcAny
    /// can only be created from its strong counterpart. The TypeId here
    /// is the type id of ManagedArc<T>.
    pub unsafe fn upgrade_any<F>(&self, index: usize, f: F) -> Option<ManagedArcAny>
    where
        F: FnOnce(PAddr, TypeId) -> Option<ManagedArcAny>,
    {
        let upgrading_obj = self.pool_items[index].lock();
        let upgrading_weak = upgrading_obj.as_ref();

        upgrading_weak.and_then(|weak| f(weak.managed_arc_inner_ptr, weak.managed_arc_type_id))
    }

    /// Like `upgrade_any`, but create the pointer using the
    /// given type.
    pub fn upgrade<T: Any>(&self, index: usize) -> Option<ManagedArc<T>>
    where
        ManagedArc<T>: Any,
    {
        let upgrading_obj = self.pool_items[index].lock();
        let upgrading_weak = upgrading_obj.as_ref();

        upgrading_weak.and_then(|weak| {
            if weak.managed_arc_type_id != TypeId::of::<ManagedArc<T>>() {
                None
            } else {
                let inner = unsafe { ManagedArc::<T>::from_ptr(weak.managed_arc_inner_ptr) };
                inner.ok()
            }
        })
    }

    /// Downgrade a strong pointer to a weak pointer and store
    /// it at `index` in this weak pool.
    pub fn downgrade_at<T: Any>(&self, arc: &ManagedArc<T>, index: usize) -> Result<(), ()>
    where
        ManagedArc<T>: Any,
    {
        let start_location = self.this_pool_location
            + (&self.pool_items[0] as *const _ as usize - self as *const _ as usize);
        let new_weak_node_addr = ManagedWeakAddr {
            weak_node_addr: start_location
                + (core::mem::size_of::<Mutex<Option<ManagedWeakNode>>>() * index),
        };
        let mut new_weak_node = ManagedWeakNode {
            managed_arc_inner_ptr: arc.managed_arc_inner_ptr,
            managed_arc_type_id: TypeId::of::<ManagedArc<T>>(),
            prev_weak_node: None,
            next_weak_node: None,
        };

        let mut weak_node_at_given_index_option = self.pool_items[index].lock();
        if weak_node_at_given_index_option.is_some() {
            return Err(());
        }

        let mut arc_inner_obj = arc.read_object();
        let arc_inner = unsafe { arc_inner_obj.as_mut() };

        let mut first_weak = arc_inner.first_weak.lock();

        if let Some(arc_second_weak_addr) = first_weak.take() {
            // ArcInner has weak. Insert the new weak as the first child.
            let second_weak_node_obj = arc_second_weak_addr.get_object();
            let mut second_weak_node_accessor = unsafe { second_weak_node_obj.as_ref().lock() };
            if let Some(ref mut second_weak_node) = *second_weak_node_accessor {
                second_weak_node.prev_weak_node = Some(new_weak_node_addr);
            } else {
                panic!("Pointer exists but the object doesn't")
            }

            new_weak_node.next_weak_node = Some(arc_second_weak_addr);
        }

        *first_weak = Some(new_weak_node_addr);
        *weak_node_at_given_index_option = Some(new_weak_node);
        Ok(())
    }

    /// Downgrade a strong pointer to a weak pointer, and then
    /// store it in a free slot in this weak pool.
    pub fn downgrade_free<T: Any>(&self, arc: &ManagedArc<T>) -> Option<usize>
    where
        ManagedArc<T>: Any,
    {
        for (i, element) in self.pool_items.iter().enumerate() {
            if element.lock().is_none() {
                let result = self.downgrade_at(arc, i);
                if result.is_err() {
                    continue;
                }
                return Some(i);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use std::mem::{self, MaybeUninit};

    use super::*;

    fn create_arc(data: u64) -> ManagedArc<u64> {
        let underlying_value: Box<MaybeUninit<ManagedArcInner<u64>>> =
            Box::new(MaybeUninit::uninit());
        let box_addr = Box::into_raw(underlying_value) as u64;
        let addr = PAddr::new(box_addr);

        unsafe { ManagedArc::new(addr, data) }
    }

    #[test]
    fn test_managed_weak_object_pool() {
        let boxed_pool = Box::new(MaybeUninit::<ManagedArcInner<ManagedWeakPool<1>>>::uninit());
        let boxed_addr = Box::into_raw(boxed_pool) as u64;
        let addr = PAddr::new(boxed_addr);

        let pool = unsafe { ManagedWeakPool1Arc::create(addr) };
        let arc1 = create_arc(999);
        let arc2 = arc1.clone();

        let pool_data = pool.read();
        pool_data.downgrade_at(&arc1, 0).unwrap();

        {
            let a2_inner = arc2.read_object();
            let val = unsafe { a2_inner.as_ref() };
            assert!(val.first_weak.lock().is_some());
        }

        mem::drop(arc1);

        // any
        let value = unsafe {
            pool_data.upgrade_any(0, |a, b| {
                assert_eq!(b, TypeId::of::<ManagedArc<u64>>());
                Some(ManagedArc::<u64>::from_ptr(a).unwrap().into())
            })
        };
        assert!(value.is_some(), "Pool cannot find this.");
        let data = value.unwrap();
        {
            let data: ManagedArc<u64> = data.into();
            let inner = data.read_object();
            let obj = unsafe { inner.as_ref() };
            assert_eq!(999, obj.arced_data);
        }

        // strong
        let value = pool_data.upgrade::<u64>(0);
        assert!(value.is_some(), "Pool cannot find this.");
        let arc3 = value.unwrap();
        {
            let inner = arc3.read_object();
            let obj = unsafe { inner.as_ref() };
            assert_eq!(999, obj.arced_data);
        }

        mem::drop(arc2);
        mem::drop(arc3);
        let value = pool_data.upgrade::<u64>(0);
        assert!(value.is_none(), "This value should have been dropped.");
    }

    #[test]
    fn test_managed_weak_object_pool_multi_weak() {
        let boxed_pool = Box::new(MaybeUninit::<ManagedArcInner<ManagedWeakPool<3>>>::uninit());
        let boxed_addr = Box::into_raw(boxed_pool) as u64;
        let addr = PAddr::new(boxed_addr);

        let pool = unsafe { ManagedWeakPool3Arc::create(addr) };
        let arc1 = create_arc(999);
        let arc2 = arc1.clone();
        assert_eq!(0, arc1.weak_count());

        let pool_data = pool.read();
        pool_data.downgrade_at(&arc1, 0).unwrap();
        pool_data.downgrade_at(&arc1, 1).unwrap();
        pool_data.downgrade_at(&arc2, 2).unwrap();
        assert_eq!(3, arc1.weak_count());

        mem::drop(arc1);

        let assert_for_index = |index: usize| {
            let value = pool_data.upgrade::<u64>(index);
            assert!(value.is_some(), "Pool cannot find this.");
            let inner = value.unwrap().read_object();
            let obj = unsafe { inner.as_ref() };
            assert_eq!(999, obj.arced_data);
        };
        assert_for_index(0);
        assert_for_index(1);
        assert_for_index(2);

        mem::drop(arc2);
        assert_eq!(None, *(*pool_data).pool_items[0].lock());
        assert_eq!(None, *(*pool_data).pool_items[1].lock());
        assert_eq!(None, *(*pool_data).pool_items[2].lock());
    }

    #[test]
    fn test_managed_weak_object_drop() {
        let boxed_pool1 = Box::new(MaybeUninit::<ManagedArcInner<ManagedWeakPool<3>>>::uninit());
        let boxed_pool2 = Box::new(MaybeUninit::<ManagedArcInner<ManagedWeakPool<3>>>::uninit());
        let boxed_addr1 = Box::into_raw(boxed_pool1) as u64;
        let boxed_addr2 = Box::into_raw(boxed_pool2) as u64;
        let addr1 = PAddr::new(boxed_addr1);
        let addr2 = PAddr::new(boxed_addr2);

        let pool1 = unsafe { ManagedWeakPool3Arc::create(addr1) };
        let pool2 = unsafe { ManagedWeakPool3Arc::create(addr2) };
        let arc1 = create_arc(999);
        let arc2 = arc1.clone();
        assert_eq!(0, arc1.weak_count());

        let pool_data1 = pool1.read();
        let pool_data2 = pool2.read();
        pool_data1.downgrade_at(&arc1, 0).unwrap();
        pool_data2.downgrade_at(&arc1, 1).unwrap();
        pool_data1.downgrade_at(&arc2, 2).unwrap();
        assert_eq!(3, arc1.weak_count());

        mem::drop(arc1);

        let assert_for_index = |pool_data: &ManagedWeakPool<3>, index: usize| {
            let value = pool_data.upgrade::<u64>(index);
            assert!(value.is_some(), "Pool cannot find this.");
            let inner = value.unwrap().read_object();
            let obj = unsafe { inner.as_ref() };
            assert_eq!(999, obj.arced_data);
        };
        assert_for_index(&*pool_data1, 0);
        assert_for_index(&*pool_data2, 1);
        assert_for_index(&*pool_data1, 2);
        assert_eq!(3, arc2.weak_count());

        mem::drop(pool_data2);
        mem::drop(pool2);

        assert_for_index(&*pool_data1, 0);
        assert_for_index(&*pool_data1, 2);
        assert_eq!(2, arc2.weak_count());
    }
}
