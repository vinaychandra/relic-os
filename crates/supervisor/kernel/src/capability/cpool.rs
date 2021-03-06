/*!
Capability Pool support

Storage for capability objects in kernel. All capability objects are
contained within the cpool. It is a fixed length array containig
[`RefCell<Capability>`]. The cpool owns the memory in which actual capability
objects are stored.
*/
use core::ops::Deref;
use std::ops::DerefMut;

use crate::util::boxed::Boxed;
use relic_abi::{cap::CapabilityErrors, prelude::CAddr};

use super::*;

/**
Capability pool kernel object.
Although cpool contains the capability objects, it itself is also another
capability object. It owns the underlying storage for capability objects.
*/
#[derive(Debug)]
pub struct Cpool {
    /**
    Owned store of capability objects.
    */
    pub cpool_data: Boxed<CpoolInner>,
    /**
    A cpool can be linked to a task. This happens when
    cpool is the root cpool for a thread.
    */
    pub linked_task: Option<StoredCap>,

    /**
    A flag set in the case that this is a derived cpool.
    In that case, data is not actually owned by this object
    but is owned by a parent cpool.
    */
    pub is_derived: bool,
}

/**
Storage for the capability objects.
*/
#[derive(Debug)]
pub struct CpoolInner {
    pub unsafe_data: [RefCell<Capability>; 256],
}

// assert the size of cpool inner.
assert_eq_size!([u8; 4096 * 4], CpoolInner);

impl Deref for Cpool {
    type Target = CpoolInner;

    fn deref(&self) -> &Self::Target {
        &self.cpool_data
    }
}

impl DerefMut for Cpool {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.cpool_data
    }
}

impl CpoolInner {
    /**
    Get a free index in the cpool. This will return a [`CapabilityErrors::CapabilitySlotsFull`]
    if there are no free indexes to be found.
    */
    pub fn get_free_index(&self) -> Result<usize, CapabilityErrors> {
        for val in self.unsafe_data.iter().enumerate() {
            if let Ok(borrow) = &val.1.try_borrow() {
                if let CapabilityEnum::EmptyCap { .. } = borrow.capability_data {
                    return Ok(val.0);
                }
            }
        }

        Err(CapabilityErrors::CapabilitySlotsFull)
    }

    /**
    Lookup a stored capability given a [`CAddr`]. This acts as if the the current
    cpool is the root cpool.
    */
    pub fn lookup(&self, caddr: CAddr) -> Option<StoredCap> {
        if caddr.1 == 0 {
            None
        } else if caddr.1 == 1 {
            let index = caddr.0[0];
            Some(unsafe { UnsafeRef::from_raw(&self.unsafe_data[index as usize]) })
        } else {
            let cur_lookup_index = caddr.0[0];
            let next_lookup_cpool = &self.unsafe_data[cur_lookup_index as usize];
            if let CapabilityEnum::Cpool(pool) = &next_lookup_cpool.borrow().capability_data {
                let next_caddr = caddr << 1;
                pool.lookup(next_caddr)
            } else {
                None
            }
        }
    }

    /**
    Search the capabilities with the given function. The will recursively go through all
    cpools and return the capability for which the user provided function returns a 0.

    The function has a depth limit after which the search fails. This is an O(n) function
    because it goes through every capability stored.

    This function also may skip capabilities if the underlying refcell is already borrowed.
    If search failed when some are already borrowed, the search returns a
    [`CapabilityErrors::CapabilitySearchFailedPartial`] instead of [`CapabilityErrors::CapabilitySearchFailed`].
    */
    pub fn search_fn<F: FnMut(StoredCap) -> bool>(
        &self,
        mut search_fn: F,
    ) -> Result<StoredCap, CapabilityErrors> {
        self.search_fn_with_depth(&mut search_fn, 0)
    }

    /**
    Search the capabilities with the given function. The will recursively go through all
    cpools and return the capability for which the user provided function returns a 0.

    The function has a depth limit after which the search fails. This is an O(n) function
    because it goes through every capability stored.

    This function also may skip capabilities if the underlying refcell is already borrowed.
    If search failed when some are already borrowed, the search returns a
    [`CapabilityErrors::CapabilitySearchFailedPartial`] instead of [`CapabilityErrors::CapabilitySearchFailed`].

    The depth parameter is used for depth tracking.
    */
    fn search_fn_with_depth<F: FnMut(StoredCap) -> bool>(
        &self,
        search_fn: &mut F,
        depth: u8,
    ) -> Result<StoredCap, CapabilityErrors> {
        if depth > 10 {
            return Err(CapabilityErrors::CapabilitySearchFailed);
        }

        let mut partial_search = false;
        self.unsafe_data
            .iter()
            .find_map(|val| {
                let cap: StoredCap = val.into();

                if cap.as_ref().try_borrow().is_err() {
                    partial_search = true;
                    return None;
                }

                if matches!(
                    cap.as_ref().borrow().capability_data,
                    CapabilityEnum::EmptyCap
                ) {
                    // Skip empty caps for faster search.
                    return None;
                }

                let cpool_search = cap
                    .as_cpool()
                    .map(|cpool| cpool.search_fn_with_depth(search_fn, depth + 1))
                    .flatten();

                if cpool_search.is_ok() {
                    return cpool_search.ok();
                }

                let user_search = search_fn(cap.clone());
                if user_search {
                    return Some(cap);
                }

                None
            })
            .ok_or_else(|| {
                if partial_search {
                    CapabilityErrors::CapabilitySearchFailedPartial
                } else {
                    CapabilityErrors::CapabilitySearchFailed
                }
            })
    }

    /**
    Write to a capability slot if the slot is empty. This will fail
    if the slot is already occupied with [`CapabilityErrors::CapabilityAlreadyOccupied.]
     */
    pub fn write_to_if_empty(
        &mut self,
        index: usize,
        cap: Capability,
    ) -> Result<StoredCap, CapabilityErrors> {
        let data_at_index = &mut self.unsafe_data[index];
        if let CapabilityEnum::EmptyCap = &data_at_index.get_mut().capability_data {
            *data_at_index = RefCell::new(cap);
            Ok(unsafe { UnsafeRef::from_raw(data_at_index) })
        } else {
            Err(CapabilityErrors::CapabilityAlreadyOccupied)
        }
    }
}

impl StoredCap {
    /**
    Create a cpool from untyped memory. This will store the created cpool
    in the provided cpool. The function returns the [`StoredCap`] pointing
    to the created cpool and an index in the cpool where this is created.
    */
    pub fn cpool_retype_from(
        untyped_memory: &mut UntypedMemory,
        cpool_to_store_in: &mut Cpool,
    ) -> Result<(StoredCap, usize), CapabilityErrors> {
        const NONE_INNER: RefCell<Capability> = RefCell::new(Capability::new());
        const NEW: CpoolInner = CpoolInner {
            unsafe_data: [NONE_INNER; 256],
        };

        let mut result_index = 0;

        let location = untyped_memory.derive(None, false, |memory: *mut CpoolInner| {
            unsafe {
                core::ptr::write(memory, NEW);
            }
            let boxed = unsafe { Boxed::new((memory as u64).into()) };

            let cpool_location_to_store = cpool_to_store_in.get_free_index()?;

            let location = cpool_to_store_in.write_to_if_empty(
                cpool_location_to_store,
                Capability {
                    capability_data: CapabilityEnum::Cpool(Cpool {
                        cpool_data: boxed,
                        linked_task: None,
                        is_derived: false,
                    }),
                    ..Default::default()
                },
            )?;

            result_index = cpool_location_to_store;
            Ok(location)
        })?;

        Ok((location, result_index))
    }

    /**
    Copy a given cpool in to the provided cpool if any. If `None`, the copy
    will be done into the source cpool.

    Returns the Stored capability and the target index.
    */
    pub fn cpool_copy(
        source_cpool: &StoredCap,
        cpool_to_store_in: Option<&StoredCap>,
    ) -> Result<(StoredCap, usize), CapabilityErrors> {
        let source_pool_accessor = source_cpool.as_cpool_mut()?;
        let new_cpool = Cpool {
            cpool_data: unsafe { source_pool_accessor.cpool_data.unsafe_clone() },
            is_derived: true,
            linked_task: None,
        };
        core::mem::drop(source_pool_accessor);

        let new_cpool_cap = Capability {
            capability_data: CapabilityEnum::Cpool(new_cpool),
            ..Default::default()
        };

        let stored_copy;
        if let Some(cpool_to_store) = cpool_to_store_in {
            let mut cpool = cpool_to_store.as_cpool_mut()?;
            let free_index = cpool.get_free_index()?;
            let result = cpool.write_to_if_empty(free_index, new_cpool_cap)?;
            stored_copy = (result, free_index);
        } else {
            let mut cpool = source_cpool.as_cpool_mut()?;
            let free_index = cpool.get_free_index()?;
            let result = cpool.write_to_if_empty(free_index, new_cpool_cap)?;
            stored_copy = (result, free_index);
        }

        source_cpool.insert_next_mem_item(&stored_copy.0);
        Ok(stored_copy)
    }
}
