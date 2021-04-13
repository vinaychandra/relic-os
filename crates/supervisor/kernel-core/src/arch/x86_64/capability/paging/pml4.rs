use std::any::Any;

use relic_abi::{cap::CapabilityErrors, SetDefault};
use spin::RwLock;

use crate::{
    addr::{PAddr, VAddr},
    arch::{
        capability::paging::page_cap::{PDCap, PDPTCap, PML4Cap, PML4Descriptor, PTCap, PageCap},
        paging::{
            table::{pd_index, pdpt_index, pml4_index, pt_index, PML4Entry, PML4},
            BASE_PAGE_LENGTH,
        },
    },
    capability::{CPoolDescriptor, UntypedDescriptor},
    prelude::MemoryObject,
    util::memory_object::{UniqueReadGuard, UniqueWriteGuard},
};

impl PML4Cap {
    /// Create a new PML4 capability in the given memory descriptor.
    pub fn retype_from(untyped: &mut UntypedDescriptor) -> Result<Self, CapabilityErrors> {
        let mut arc: Option<Self> = None;

        let start_paddr = unsafe { untyped.allocate(BASE_PAGE_LENGTH, BASE_PAGE_LENGTH)? };

        unsafe {
            untyped.derive(
                Self::inner_type_length(),
                Self::inner_type_alignment(),
                |paddr, next_child| {
                    let mut desc = PML4Descriptor {
                        start_paddr,
                        next: next_child,
                    };

                    for item in desc.write().iter_mut() {
                        *item = PML4Entry::empty();
                    }

                    // TODO vinay
                    // desc.write()[pml4_index(VAddr::from(KERNEL_BASE))] =
                    //     PML4Entry::new(KERNEL_PDPT.paddr(), PML4_P | PML4_RW);

                    arc = Some(Self::new(paddr, RwLock::new(desc)));

                    arc.clone().unwrap().into()
                },
            )?;
        }

        Ok(arc.unwrap())
    }

    pub fn map_pdpt(&mut self, index: usize, sub: &PDPTCap) -> Result<(), CapabilityErrors> {
        let mut current_desc = self.write();
        let mut current = current_desc.write();
        let sub_desc = sub.read();
        // TODO vinay
        // assert!(!(pml4_index(VAddr::from(KERNEL_BASE)) == index));
        if !current[index].is_present() {
            return Err(CapabilityErrors::CapabilityAlreadyOccupied);
        }

        sub_desc
            .mapped_weak_pool
            .read()
            .downgrade_at(self, 0)
            .map_err(|_| CapabilityErrors::MemoryAlreadyMapped)?;
        current[index] = PML4Entry::new(
            sub_desc.start_paddr(),
            PML4Entry::PML4_P | PML4Entry::PML4_RW | PML4Entry::PML4_US,
        );

        Ok(())
    }

    pub fn map<T: SetDefault + Any>(
        &mut self,
        vaddr: VAddr,
        page: &PageCap<T>,
        untyped: &mut UntypedDescriptor,
        cpool: &mut CPoolDescriptor,
    ) -> Result<(), CapabilityErrors> {
        let mut pdpt_cap: PDPTCap = {
            let index = pml4_index(vaddr);

            if !self.read().read()[index].is_present() {
                let pdpt_cap = PDPTCap::retype_from(untyped)?;
                cpool.downgrade_free(&pdpt_cap)?;
                self.map_pdpt(index, &pdpt_cap)?;
            }

            let position = (0..cpool.size())
                .position(|i| {
                    let any = cpool.upgrade_any(i);
                    if let Some(any) = any {
                        if any.is::<PDPTCap>() {
                            let cap: PDPTCap = any.into();
                            let cap_desc = cap.read();
                            cap_desc.start_paddr() == { self.read().read()[index] }.get_address()
                        } else {
                            crate::capability::drop_any(any);
                            false
                        }
                    } else {
                        false
                    }
                })
                .ok_or(CapabilityErrors::CapabilitySearchFailed)?;

            cpool.upgrade(position).unwrap()
        };

        let mut pd_cap: PDCap = {
            let index = pdpt_index(vaddr);

            if !pdpt_cap.read().read()[index].is_present() {
                let pd_cap = PDCap::retype_from(untyped)?;
                cpool.downgrade_free(&pd_cap)?;
                pdpt_cap.map_pd(index, &pd_cap)?;
            }

            let position = (0..cpool.size())
                .position(|i| {
                    let any = cpool.upgrade_any(i);
                    if let Some(any) = any {
                        if any.is::<PDCap>() {
                            let cap: PDCap = any.into();
                            let cap_desc = cap.read();
                            cap_desc.start_paddr() == { pdpt_cap.read().read()[index] }
                                .get_address()
                        } else {
                            crate::capability::drop_any(any);
                            false
                        }
                    } else {
                        false
                    }
                })
                .ok_or(CapabilityErrors::CapabilitySearchFailed)?;

            cpool.upgrade(position).unwrap()
        };

        let mut pt_cap: PTCap = {
            let index = pd_index(vaddr);

            if !pd_cap.read().read()[index].is_present() {
                let pt_cap = PTCap::retype_from(untyped)?;
                cpool.downgrade_free(&pt_cap)?;
                pd_cap.map_pt(index, &pt_cap)?;
            }

            let position = (0..cpool.size())
                .position(|i| {
                    let any = cpool.upgrade_any(i);
                    if let Some(any) = any {
                        if any.is::<PTCap>() {
                            let cap: PTCap = any.into();
                            let cap_desc = cap.read();
                            cap_desc.start_paddr() == { pd_cap.read().read()[index] }.get_address()
                        } else {
                            crate::capability::drop_any(any);
                            false
                        }
                    } else {
                        false
                    }
                })
                .ok_or(CapabilityErrors::CapabilitySearchFailed)?;

            cpool.upgrade(position).unwrap()
        };

        pt_cap.map_page(pt_index(vaddr), page)?;
        Ok(())
    }
}

impl PML4Descriptor {
    pub fn start_paddr(&self) -> PAddr {
        self.start_paddr
    }

    pub fn length(&self) -> usize {
        BASE_PAGE_LENGTH
    }

    fn page_object(&self) -> MemoryObject<PML4> {
        unsafe { MemoryObject::new(self.start_paddr) }
    }

    pub fn read(&self) -> UniqueReadGuard<PML4> {
        unsafe { UniqueReadGuard::new(self.page_object()) }
    }

    fn write(&mut self) -> UniqueWriteGuard<PML4> {
        unsafe { UniqueWriteGuard::new(self.page_object()) }
    }

    pub fn switch_to(&mut self) {
        use crate::arch::paging;

        unsafe {
            paging::utils::switch_to(self.start_paddr);
        }
    }
}
