use crate::util::boxed::Boxed;
use relic_abi::{cap::CapabilityErrors, prelude::CAddr};

use super::*;

#[derive(Debug)]
pub struct CPoolInner {
    unsafe_data: [Capability; 256],
}

impl CPoolInner {
    pub fn get_free_index(&self) -> Result<usize, CapabilityErrors> {
        for val in self.unsafe_data.iter().enumerate() {
            if let &CapabilityEnum::EmptyCap { .. } = &*val.1.capability_data.borrow() {
                return Ok(val.0);
            }
        }

        Err(CapabilityErrors::CapabilitySlotsFull)
    }

    pub fn lookup(&self, caddr: CAddr) -> Option<UnsafeRef<Capability>> {
        if caddr.1 == 0 {
            None
        } else if caddr.1 == 1 {
            let index = caddr.0[0];
            Some(unsafe { UnsafeRef::from_raw(&self.unsafe_data[index as usize]) })
        } else {
            let cur_lookup_index = caddr.0[0];
            let next_lookup_cpool = &self.unsafe_data[cur_lookup_index as usize];
            if let CapabilityEnum::CPool { data, .. } = &*next_lookup_cpool.capability_data.borrow()
            {
                let next_caddr = caddr << 1;
                data.lookup(next_caddr)
            } else {
                None
            }
        }
    }

    pub fn write_to_if_empty(
        &mut self,
        index: usize,
        cap: Capability,
    ) -> Result<UnsafeRef<Capability>, CapabilityErrors> {
        let data_at_index = &mut self.unsafe_data[index];
        if let CapabilityEnum::EmptyCap = &*data_at_index.capability_data.get_mut() {
            *data_at_index = cap;
            Ok(unsafe { UnsafeRef::from_raw(data_at_index) })
        } else {
            Err(CapabilityErrors::CapabilityAlreadyOccupied)
        }
    }
}

impl Drop for CPoolInner {
    fn drop(&mut self) {
        todo!("CPoolInner drop not supproted")
    }
}

impl Capability {
    pub fn cpool_retype_from(
        untyped_memory: &Capability,
        cpool_to_store_in: &Capability,
    ) -> Result<(UnsafeRef<Capability>, usize), CapabilityErrors> {
        const NONE_INNER: Capability = Capability {
            capability_data: RefCell::new(CapabilityEnum::EmptyCap),
            mem_tree_link: LinkedListLink::new(),
            paging_tree_link: LinkedListLink::new(),
        };

        let new = CPoolInner {
            unsafe_data: [NONE_INNER; 256],
        };

        let mut result_index = 0;
        let mut boxed2 = None;

        untyped_memory.untyped_derive(|memory: *mut CPoolInner| {
            unsafe {
                core::ptr::write(memory, new);
            }
            let boxed = unsafe { Boxed::new((memory as u64).into()) };

            let cpool_location_to_store: usize;
            {
                let cpool_store_borrow_mut = cpool_to_store_in.capability_data.borrow();
                let cpool_storage_inner;
                if let CapabilityEnum::CPool { data } = &*cpool_store_borrow_mut {
                    cpool_storage_inner = data
                } else {
                    return Err(CapabilityErrors::CapabilityMismatch);
                }

                cpool_location_to_store = cpool_storage_inner.get_free_index()?;
            }

            let mut cpool_store_borrow_mut = cpool_to_store_in.capability_data.borrow_mut();
            let cpool_storage_inner;
            if let CapabilityEnum::CPool { data } = &mut *cpool_store_borrow_mut {
                cpool_storage_inner = data
            } else {
                return Err(CapabilityErrors::CapabilityMismatch);
            }

            let location = cpool_storage_inner.write_to_if_empty(
                cpool_location_to_store,
                Capability {
                    mem_tree_link: LinkedListLink::new(),
                    capability_data: RefCell::new(CapabilityEnum::CPool { data: boxed }),
                    paging_tree_link: LinkedListLink::new(),
                },
            )?;

            boxed2 = Some(location.clone());
            result_index = cpool_location_to_store;
            Ok(location)
        })?;

        Ok((boxed2.unwrap(), result_index))
    }
}
