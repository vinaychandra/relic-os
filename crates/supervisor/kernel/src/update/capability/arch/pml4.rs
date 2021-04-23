use relic_abi::cap::CapabilityErrors;

use super::*;
use crate::{arch::globals::BASE_PAGE_LENGTH, util::boxed::Boxed};

pub struct PML4Read<'a> {
    page_data: &'a Boxed<[PML4Entry; 512]>,
    children: &'a LinkedList<PagingTreeAdapter>,
}

pub struct PML4Write<'a> {
    page_data: &'a mut Boxed<[PML4Entry; 512]>,
    children: &'a mut LinkedList<PagingTreeAdapter>,
}

impl Capability {
    pub fn pml4_create(&self) -> Option<PML4Read<'_>> {
        if let CapabilityEnum::Arch(ArchCap::PML4 {
            page_data,
            children,
        }) = &self.capability_data
        {
            Some(PML4Read {
                page_data,
                children,
            })
        } else {
            None
        }
    }

    pub fn pml4_create_mut(&mut self) -> Option<PML4Write<'_>> {
        if let CapabilityEnum::Arch(ArchCap::PML4 {
            page_data,
            children,
        }) = &mut self.capability_data
        {
            Some(PML4Write {
                page_data,
                children,
            })
        } else {
            None
        }
    }

    pub fn pml4_retype_from(
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
                core::ptr::write(memory, [PML4Entry::empty(); 512]);
            }
            let boxed = unsafe { Boxed::new((memory as u64).into()) };

            let stored_index = cpool_to_store_in.read().get_free_index()?;
            let cap = cpool_to_store_in.write_to_if_empty(
                stored_index,
                Capability {
                    mem_tree_link: LinkedListLink::new(),
                    paging_tree_link: LinkedListLink::new(),
                    capability_data: CapabilityEnum::Arch(ArchCap::PML4 {
                        children: LinkedList::new(PagingTreeAdapter::new()),
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

impl<'a> PML4Read<'a> {
    pub fn start_paddr(&self) -> PAddrGlobal {
        self.page_data.paddr_global()
    }

    pub fn length(&self) -> usize {
        BASE_PAGE_LENGTH
    }
}

impl<'a> PML4Write<'a> {
    pub fn read(&self) -> PML4Read<'_> {
        PML4Read {
            children: self.children,
            page_data: self.page_data,
        }
    }
}

impl Capability {
    pub fn pml4_map_pdpt(
        &mut self,
        index: usize,
        pdpt_page: &mut Capability,
    ) -> Result<(), CapabilityErrors> {
        let unsafe_self = unsafe { UnsafeRef::from_raw(self) };
        let writer = self
            .pml4_create_mut()
            .ok_or(CapabilityErrors::CapabilityMismatch)?;

        let pdpt_write = pdpt_page
            .pdpt_create_mut()
            .ok_or(CapabilityErrors::CapabilityMismatch)?;

        if writer.page_data[index].is_present() {
            return Err(CapabilityErrors::MemoryAlreadyMapped);
        }

        if pdpt_write.parent_pml4.is_some() {
            return Err(CapabilityErrors::MemoryAlreadyMapped);
        }

        writer.page_data[index] = PML4Entry::new(
            pdpt_write.read().start_paddr().to_paddr(),
            PML4Entry::PRESENT | PML4Entry::READ_WRITE | PML4Entry::USERSPACE,
        );

        *pdpt_write.parent_pml4 = Some(unsafe_self);
        let refcell = unsafe { UnsafeRef::from_raw(pdpt_page) };
        writer.children.push_front(refcell);

        Ok(())
    }
}
