use crate::{
    addr::PAddrGlobal,
    util::managed_arc::{
        ManagedArc, ManagedArcAny, ManagedArcInner, ManagedWeakAddr, ManagedWeakNode,
    },
};
use spin::Mutex;
use std::{
    any::{Any, TypeId},
    mem::MaybeUninit,
    ptr::{self, NonNull},
};

/// A pool of managed weak nodes.
#[derive(Debug)]
pub struct ManagedWeakPool<const SIZE: usize> {
    pool_items: [Mutex<Option<ManagedWeakNode>>; SIZE],
    this_pool_location: PAddrGlobal,
}

/// Managed Arc for weak pool of size 1.
pub type ManagedWeakPool1Arc = ManagedArc<ManagedWeakPool<1>>;
/// Managed Arc for weak pool of size 3.
pub type ManagedWeakPool3Arc = ManagedArc<ManagedWeakPool<3>>;
/// Managed Arc for weak pool of size 256.
pub type ManagedWeakPool256Arc = ManagedArc<ManagedWeakPool<256>>;

impl<const SIZE: usize> ManagedArc<ManagedWeakPool<SIZE>> {
    /// Create a managed weak pool in the given physical address.
    pub unsafe fn create(pool_addr: PAddrGlobal) -> Self {
        let mut arc: Self = ManagedArc::new(pool_addr, MaybeUninit::uninit().assume_init());
        let inner = arc.read_object_mut();

        // Determine the offset of the object within the inner
        let offset = (&inner.arced_data as *const _ as usize) - (inner as *const _ as usize);

        ptr::write(&mut inner.arced_data.this_pool_location, pool_addr + offset);
        for element in inner.arced_data.pool_items.iter_mut() {
            ptr::write(element, Mutex::new(None));
        }

        arc
    }
}

impl<const SIZE: usize> ManagedWeakPool<SIZE> {
    /// Create a new strong pointer if `index` points to a
    /// non-none weak pointer in the weak pool.
    pub fn upgrade_any(&self, index: usize) -> Option<ManagedArcAny> {
        let upgrading_obj = self.pool_items[index].lock();
        let upgrading_weak = upgrading_obj.as_ref()?;

        let inner = upgrading_weak.get_inner();
        ManagedArc::with_inner(inner).ok()
    }

    /// Like `upgrade_any`, but create the pointer using the
    /// given type.
    pub fn upgrade<T: Any>(&self, index: usize) -> Option<ManagedArc<T>>
    where
        ManagedArc<T>: Any,
        T: Sized,
    {
        let upgrading_obj = self.pool_items[index].lock();

        if let Some(ref weak) = *upgrading_obj {
            let inner = weak.get_inner();
            let dyn_inner = &inner.arced_data;
            if (*dyn_inner).type_id() != TypeId::of::<T>() {
                None
            } else {
                let data = unsafe {
                    &*(inner as *const ManagedArcInner<dyn Any> as *const ManagedArcInner<T>)
                };
                ManagedArc::<T>::with_inner(data).ok()
            }
        } else {
            None
        }
    }

    /// Downgrade any managed arc and store it at `index` in this weak pool.
    pub fn downgrade_at(&self, arc: ManagedArcAny, index: usize) -> Result<(), ()> {
        let mutex = &self.pool_items[index];
        let mut mutex_guard = mutex.lock();
        if mutex_guard.is_some() {
            return Err(());
        }

        let this_weak_node_addr: NonNull<Mutex<Option<ManagedWeakNode>>> = mutex.into();
        let new_weak_node_addr = ManagedWeakAddr {
            weak_node_addr: this_weak_node_addr.into(),
        };
        let mut arc_any: ManagedArcAny = arc;
        let mut new_weak_node = ManagedWeakNode {
            ptr: arc_any.managed_arc_inner_ptr,
            prev_weak_node: None,
            next_weak_node: None,
        };

        let arc_inner = unsafe { arc_any.read_object_mut() };
        let mut first_weak = arc_inner.first_weak.lock();

        if let Some(arc_second_weak_addr) = first_weak.take() {
            // ArcInner has weak. Insert the new weak as the first child.
            let second_weak_node_obj = arc_second_weak_addr.get_object();
            let mut second_weak_node_accessor = second_weak_node_obj.lock();
            if let Some(ref mut second_weak_node) = *second_weak_node_accessor {
                second_weak_node.prev_weak_node = Some(new_weak_node_addr);
            } else {
                panic!("Pointer exists but the object doesn't")
            }

            new_weak_node.next_weak_node = Some(arc_second_weak_addr);
        }

        *first_weak = Some(new_weak_node_addr);
        *mutex_guard = Some(new_weak_node);
        Ok(())
    }

    /// Downgrade a strong pointer to a weak pointer, and then
    /// store it in a free slot in this weak pool.
    pub fn downgrade_free(&self, arc: ManagedArcAny) -> Option<usize> {
        for (i, element) in self.pool_items.iter().enumerate() {
            if element.lock().is_none() {
                let result = self.downgrade_at(arc.clone(), i);
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

    use crate::util::managed_arc::ManagedArcInner;

    use super::*;

    fn create_arc(data: u64) -> ManagedArc<u64> {
        let underlying_value: Box<MaybeUninit<ManagedArcInner<u64>>> =
            Box::new(MaybeUninit::uninit());
        let box_addr = Box::into_raw(underlying_value) as u64;
        let addr = PAddrGlobal::new(box_addr);

        unsafe { ManagedArc::new(addr, data) }
    }

    #[test]
    fn test_managed_weak_object_pool() {
        let boxed_pool = Box::new(MaybeUninit::<ManagedArcInner<ManagedWeakPool<1>>>::uninit());
        let boxed_addr = Box::into_raw(boxed_pool) as u64;
        let addr = PAddrGlobal::new(boxed_addr);

        let pool = unsafe { ManagedWeakPool1Arc::create(addr) };
        let arc1 = create_arc(999);
        let arc2 = arc1.clone();

        pool.downgrade_at(arc1.clone(), 0).unwrap();

        {
            let a2_inner = arc2.read_object();
            assert!(a2_inner.first_weak.lock().is_some());
        }

        mem::drop(arc1);

        // any
        let value = pool.upgrade_any(0);
        assert!(value.is_some(), "Pool cannot find this.");
        let data = value.unwrap();
        {
            let data: ManagedArc<u64> = data.into();
            let inner = data.read_object();
            assert_eq!(999, inner.arced_data);
        }

        // strong
        let value = pool.upgrade::<u64>(0);
        assert!(value.is_some(), "Pool cannot find this.");
        let arc3 = value.unwrap();
        {
            let inner = arc3.read_object();
            assert_eq!(999, inner.arced_data);
        }

        mem::drop(arc2);
        mem::drop(arc3);
        let value = pool.upgrade::<u64>(0);
        assert!(value.is_none(), "This value should have been dropped.");
    }

    #[test]
    fn test_managed_weak_object_pool_multi_weak() {
        let boxed_pool = Box::new(MaybeUninit::<ManagedArcInner<ManagedWeakPool<3>>>::uninit());
        let boxed_addr = Box::into_raw(boxed_pool) as u64;
        let addr = PAddrGlobal::new(boxed_addr);

        let pool = unsafe { ManagedWeakPool3Arc::create(addr) };
        let arc1 = create_arc(999);
        let arc2 = arc1.clone();
        assert_eq!(0, arc1.weak_count());

        pool.downgrade_at(arc1.clone(), 0).unwrap();
        pool.downgrade_at(arc1.clone(), 1).unwrap();
        pool.downgrade_at(arc2.clone(), 2).unwrap();
        assert_eq!(3, arc1.weak_count());

        mem::drop(arc1);

        let assert_for_index = |index: usize| {
            let value = pool.upgrade::<u64>(index);
            assert!(value.is_some(), "Pool cannot find this.");
            let inner = value.unwrap();
            assert_eq!(999, *inner);
        };
        assert_for_index(0);
        assert_for_index(1);
        assert_for_index(2);

        mem::drop(arc2);
        assert_eq!(None, *(*pool).pool_items[0].lock());
        assert_eq!(None, *(*pool).pool_items[1].lock());
        assert_eq!(None, *(*pool).pool_items[2].lock());
    }

    #[test]
    fn test_managed_weak_object_drop() {
        let boxed_pool1 = Box::new(MaybeUninit::<ManagedArcInner<ManagedWeakPool<3>>>::uninit());
        let boxed_pool2 = Box::new(MaybeUninit::<ManagedArcInner<ManagedWeakPool<3>>>::uninit());
        let boxed_addr1 = Box::into_raw(boxed_pool1) as u64;
        let boxed_addr2 = Box::into_raw(boxed_pool2) as u64;
        let addr1 = PAddrGlobal::new(boxed_addr1);
        let addr2 = PAddrGlobal::new(boxed_addr2);

        let pool1 = unsafe { ManagedWeakPool3Arc::create(addr1) };
        let pool2 = unsafe { ManagedWeakPool3Arc::create(addr2) };
        let arc1 = create_arc(999);
        let arc2 = arc1.clone();
        assert_eq!(0, arc1.weak_count());

        pool1.downgrade_at(arc1.clone(), 0).unwrap();
        pool2.downgrade_at(arc1.clone(), 1).unwrap();
        pool1.downgrade_at(arc2.clone(), 2).unwrap();
        assert_eq!(3, arc1.weak_count());

        mem::drop(arc1);

        let assert_for_index = |pool_data: &ManagedWeakPool<3>, index: usize| {
            let value = pool_data.upgrade::<u64>(index);
            assert!(value.is_some(), "Pool cannot find this.");
            assert_eq!(999, *value.unwrap());
        };
        assert_for_index(&*pool1, 0);
        assert_for_index(&*pool2, 1);
        assert_for_index(&*pool1, 2);
        assert_eq!(3, arc2.weak_count());

        mem::drop(pool2);

        assert_for_index(&*pool1, 0);
        assert_for_index(&*pool1, 2);
        assert_eq!(2, arc2.weak_count());
    }
}
