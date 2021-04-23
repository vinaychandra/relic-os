use relic_abi::cap::CapabilityErrors;

use super::*;
use crate::{arch::globals::BASE_PAGE_LENGTH, util::boxed::Boxed};

pub struct PDRead<'a> {
    pub page_data: &'a Boxed<[PDEntry; 512]>,
    pub parent_pml4: &'a Option<UnsafeRef<Capability>>,
}

pub struct PDWrite<'a> {
    pub page_data: &'a mut Boxed<[PDEntry; 512]>,
    pub parent_pml4: &'a mut Option<UnsafeRef<Capability>>,
}

impl Capability {
    pub fn pd_create(&self) -> Option<PDRead<'_>> {
        if let CapabilityEnum::Arch(ArchCap::PD {
            page_data,
            parent_pml4,
        }) = &self.capability_data
        {
            Some(PDRead {
                page_data,
                parent_pml4,
            })
        } else {
            None
        }
    }

    pub fn pd_create_mut(&mut self) -> Option<PDWrite<'_>> {
        if let CapabilityEnum::Arch(ArchCap::PD {
            page_data,
            parent_pml4,
        }) = &mut self.capability_data
        {
            Some(PDWrite {
                page_data,
                parent_pml4,
            })
        } else {
            None
        }
    }

    pub fn pd_retype_from(
        untyped: &mut Capability,
        cpool_to_store_in: &mut CPoolWrite,
    ) -> Result<(UnsafeRef<Capability>, usize), CapabilityErrors> {
        let mut untyped_data = untyped
            .untyped_create_mut()
            .ok_or(CapabilityErrors::CapabilityMismatch)?;

        let mut result_index = 0;
        let mut boxed2 = None;

        untyped_data.derive(|memory| {
            unsafe {
                core::ptr::write(memory, [PDEntry::empty(); 512]);
            }
            let boxed = unsafe { Boxed::new((memory as u64).into()) };

            let stored_index = cpool_to_store_in.read().get_free_index()?;
            let cap = cpool_to_store_in.write_to_if_empty(
                stored_index,
                Capability {
                    mem_tree_link: LinkedListLink::new(),
                    paging_tree_link: LinkedListLink::new(),
                    capability_data: CapabilityEnum::Arch(ArchCap::PD {
                        parent_pml4: None,
                        page_data: boxed,
                    }),
                },
            )?;

            result_index = stored_index;
            boxed2 = Some(cap.clone());
            Ok(cap)
        })?;

        Ok((boxed2.unwrap(), result_index))
    }
}

impl<'a> PDRead<'a> {
    pub fn start_paddr(&self) -> PAddrGlobal {
        self.page_data.paddr_global()
    }

    pub fn length(&self) -> usize {
        BASE_PAGE_LENGTH
    }
}

impl<'a> PDWrite<'a> {
    pub fn read(&self) -> PDRead<'_> {
        PDRead {
            parent_pml4: self.parent_pml4,
            page_data: self.page_data,
        }
    }
}

// impl Capability {
//     pub fn PD_map_PD(
//         &mut self,
//         index: usize,
//         sub: &mut Capability,
//     ) -> Result<(), CapabilityErrors> {
//         let writer = self
//             .PD_create_mut()
//             .ok_or(CapabilityErrors::CapabilityMismatch)?;

//         if writer.page_data[index].is_present() {
//             return Err(CapabilityErrors::MemoryAlreadyMapped);
//         }

//         Ok(())
//     }
// }
