use crate::util::boxed::Boxed;
use relic_abi::{cap::CapabilityErrors, prelude::CAddr};

use super::*;

#[derive(Debug)]
pub struct Cpool {
    pub data: Boxed<CpoolInner>,
}

#[derive(Debug)]
pub struct CpoolInner {
    pub unsafe_data: [RefCell<Capability>; 256],
}

impl Cpool {
    /// Size of the capability pool.
    pub fn size(&self) -> usize {
        self.data.unsafe_data.len()
    }

    pub fn get_free_index(&self) -> Result<usize, CapabilityErrors> {
        for val in self.data.unsafe_data.iter().enumerate() {
            if let &CapabilityEnum::EmptyCap { .. } = &val.1.borrow().capability_data {
                return Ok(val.0);
            }
        }

        Err(CapabilityErrors::CapabilitySlotsFull)
    }

    pub fn lookup(&self, caddr: CAddr) -> Option<StoredCap> {
        if caddr.1 == 0 {
            None
        } else if caddr.1 == 1 {
            let index = caddr.0[0];
            Some(unsafe { UnsafeRef::from_raw(&self.data.unsafe_data[index as usize]) })
        } else {
            let cur_lookup_index = caddr.0[0];
            let next_lookup_cpool = &self.data.unsafe_data[cur_lookup_index as usize];
            if let CapabilityEnum::Cpool(pool) = &next_lookup_cpool.borrow().capability_data {
                let next_caddr = caddr << 1;
                pool.lookup(next_caddr)
            } else {
                None
            }
        }
    }

    pub fn lookup_index_unsafe(&self, index: usize) -> StoredCap {
        unsafe { UnsafeRef::from_raw(&self.data.unsafe_data[index as usize]) }
    }

    pub fn write_to_if_empty(
        &mut self,
        index: usize,
        cap: Capability,
    ) -> Result<StoredCap, CapabilityErrors> {
        let data_at_index = &mut self.data.unsafe_data[index];
        if let CapabilityEnum::EmptyCap = &data_at_index.get_mut().capability_data {
            *data_at_index = RefCell::new(cap);
            Ok(unsafe { UnsafeRef::from_raw(data_at_index) })
        } else {
            Err(CapabilityErrors::CapabilityAlreadyOccupied)
        }
    }
}

impl Drop for CpoolInner {
    fn drop(&mut self) {
        todo!("CpoolInner drop not supproted")
    }
}

impl StoredCap {
    pub fn cpool_retype_from(
        untyped_memory: &mut UntypedMemory,
        cpool_to_store_in: &mut Cpool,
    ) -> Result<(StoredCap, usize), CapabilityErrors> {
        const NONE_INNER: RefCell<Capability> = RefCell::new(Capability::new());

        let new = CpoolInner {
            unsafe_data: [NONE_INNER; 256],
        };

        let mut result_index = 0;

        let location = untyped_memory.derive(|memory: *mut CpoolInner| {
            unsafe {
                core::ptr::write(memory, new);
            }
            let boxed = unsafe { Boxed::new((memory as u64).into()) };

            let cpool_location_to_store = cpool_to_store_in.get_free_index()?;

            let location = cpool_to_store_in.write_to_if_empty(
                cpool_location_to_store,
                Capability {
                    capability_data: CapabilityEnum::Cpool(Cpool { data: boxed }),
                    ..Default::default()
                },
            )?;

            result_index = cpool_location_to_store;
            Ok(location)
        })?;

        Ok((location, result_index))
    }
}
