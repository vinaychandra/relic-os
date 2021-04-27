use std::any::TypeId;

use relic_abi::cap::CapabilityErrors;

use super::*;
use crate::{arch::globals::BASE_PAGE_LENGTH, util::boxed::Boxed};

#[derive(Debug)]
pub struct RawPage {
    page_data: Boxed<Inner>,
    pub type_id: TypeId,

    pub next_paging_item: Option<StoredCap>,
    pub prev_paging_item: Option<StoredCap>,
}

#[repr(align(4096))]
#[derive(Debug)]
struct Inner([u8; 4096]);

impl StoredCap {
    pub fn raw_page_retype_from<T: 'static>(
        untyped: &mut UntypedMemory,
        cpool_to_store_in: &mut Cpool,
    ) -> Result<(StoredCap, usize), CapabilityErrors> {
        assert!(core::mem::size_of::<T>() <= 4096);
        assert!(core::mem::align_of::<T>() <= 4096);
        let mut result_index = 0;

        let cap = untyped.derive(|memory| {
            unsafe {
                core::ptr::write(memory, Inner([0u8; 4096]));
            }
            let boxed = unsafe { Boxed::new((memory as u64).into()) };

            let stored_index = cpool_to_store_in.get_free_index()?;
            let cap = cpool_to_store_in.write_to_if_empty(
                stored_index,
                Capability {
                    capability_data: CapabilityEnum::RawPage(RawPage {
                        next_paging_item: None,
                        prev_paging_item: None,
                        page_data: boxed,
                        type_id: TypeId::of::<T>(),
                    }),
                    ..Default::default()
                },
            )?;

            result_index = stored_index;
            Ok(cap)
        })?;

        Ok((cap, result_index))
    }
}

impl RawPage {
    pub fn start_paddr(&self) -> PAddrGlobal {
        self.page_data.paddr_global()
    }

    pub fn length(&self) -> usize {
        BASE_PAGE_LENGTH
    }

    pub fn page_data<T: 'static>(&self) -> &T {
        assert!(TypeId::of::<T>() == self.type_id);
        unsafe { &*(&self.page_data.0[0] as *const u8 as *const T) }
    }

    pub fn page_data_mut<T: 'static>(&mut self) -> &mut T {
        assert!(TypeId::of::<T>() == self.type_id);
        unsafe { &mut *(&self.page_data.0[0] as *const u8 as *mut T) }
    }
}
