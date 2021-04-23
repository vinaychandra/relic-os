use relic_abi::cap::CapabilityErrors;

use super::*;
use crate::{arch::globals::BASE_PAGE_LENGTH, util::boxed::Boxed};

pub struct PDPTRead<'a> {
    pub page_data: &'a Boxed<[PDPTEntry; 512]>,
    pub parent_pml4: &'a Option<UnsafeRef<Capability>>,
}

pub struct PDPTWrite<'a> {
    pub page_data: &'a mut Boxed<[PDPTEntry; 512]>,
    pub parent_pml4: &'a mut Option<UnsafeRef<Capability>>,
}

impl Capability {
    pub fn pdpt_create(&self) -> Option<PDPTRead<'_>> {
        if let CapabilityEnum::Arch(ArchCap::PDPT {
            page_data,
            parent_pml4,
        }) = &self.capability_data
        {
            Some(PDPTRead {
                page_data,
                parent_pml4,
            })
        } else {
            None
        }
    }

    pub fn pdpt_create_mut(&mut self) -> Option<PDPTWrite<'_>> {
        if let CapabilityEnum::Arch(ArchCap::PDPT {
            page_data,
            parent_pml4,
        }) = &mut self.capability_data
        {
            Some(PDPTWrite {
                page_data,
                parent_pml4,
            })
        } else {
            None
        }
    }

    pub fn pdpt_retype_from(
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
                core::ptr::write(memory, [PDPTEntry::empty(); 512]);
            }
            let boxed = unsafe { Boxed::new((memory as u64).into()) };

            let stored_index = cpool_to_store_in.read().get_free_index()?;
            let cap = cpool_to_store_in.write_to_if_empty(
                stored_index,
                Capability {
                    mem_tree_link: LinkedListLink::new(),
                    paging_tree_link: LinkedListLink::new(),
                    capability_data: CapabilityEnum::Arch(ArchCap::PDPT {
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

impl<'a> PDPTRead<'a> {
    pub fn start_paddr(&self) -> PAddrGlobal {
        self.page_data.paddr_global()
    }

    pub fn length(&self) -> usize {
        BASE_PAGE_LENGTH
    }
}

impl<'a> PDPTWrite<'a> {
    pub fn read(&self) -> PDPTRead<'_> {
        PDPTRead {
            parent_pml4: self.parent_pml4,
            page_data: self.page_data,
        }
    }
}

impl Capability {
    pub fn pdpt_map_pdpt(
        &mut self,
        index: usize,
        pd_page: &mut Capability,
    ) -> Result<(), CapabilityErrors> {
        let writer = self
            .pdpt_create_mut()
            .ok_or(CapabilityErrors::CapabilityMismatch)?;

        let pd_write = pd_page
            .pd_create_mut()
            .ok_or(CapabilityErrors::CapabilityMismatch)?;

        if writer.page_data[index].is_present() {
            return Err(CapabilityErrors::MemoryAlreadyMapped);
        }

        if pd_write.parent_pml4.is_some() {
            return Err(CapabilityErrors::MemoryAlreadyMapped);
        }

        writer.page_data[index] = PDPTEntry::new(
            pd_write.read().start_paddr().to_paddr(),
            PDPTEntry::PRESENT | PDPTEntry::READ_WRITE | PDPTEntry::USERSPACE,
        );

        // *pd_write.parent_pml4 = writer.parent_pml4.clone();
        // let refcell = unsafe { UnsafeRef::from_raw(pd_page) };

        // if let Some(inner) = writer.parent_pml4 {
        //     let mut pml4 = inner.clone();
        //     let pml4_write = pml4.pml4_create_mut();
        // }

        Ok(())
    }
}
