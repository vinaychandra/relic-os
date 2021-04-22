use std::{any::Any, mem, ptr::NonNull};

use relic_abi::{cap::CapabilityErrors, SetDefault};
use spin::RwLock;

use crate::{
    addr::{PAddrGlobal, VAddr},
    arch::{
        capability::paging::page_cap::{PDCap, PDPTCap, PML4Cap, PML4Descriptor, PTCap, PageCap},
        globals::BASE_PAGE_LENGTH,
        paging::{
            table::{pd_index, pdpt_index, pml4_index, pt_index, PML4Entry, PML4},
            utils,
        },
    },
    capability::{CPoolDescriptor, UntypedDescriptor},
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

                    {
                        let current_page_table_paddr: u64 = utils::cr3().into();
                        let current_page_table: &PML4 = &*(current_page_table_paddr as *const _);
                        desc.write()[510] = current_page_table[510];
                        desc.write()[511] = current_page_table[511];
                    }

                    arc = Some(Self::new(paddr, RwLock::new(desc)));

                    arc.clone().unwrap()
                },
            )?;
        }

        Ok(arc.unwrap())
    }

    pub fn map_pdpt(&mut self, index: usize, sub: &PDPTCap) -> Result<(), CapabilityErrors> {
        let mut current_desc = self.write();
        let current = current_desc.write();
        let sub_desc = sub.read();
        // TODO vinay
        // assert!(!(pml4_index(VAddr::from(KERNEL_BASE)) == index));
        if current[index].is_present() {
            return Err(CapabilityErrors::CapabilityAlreadyOccupied);
        }

        sub_desc
            .mapped_weak_pool
            .downgrade_at(self.clone(), 0)
            .map_err(|_| CapabilityErrors::MemoryAlreadyMapped)?;
        current[index] = PML4Entry::new(
            sub_desc.start_paddr().to_paddr(),
            PML4Entry::PRESENT | PML4Entry::READ_WRITE | PML4Entry::USERSPACE,
        );

        Ok(())
    }

    pub fn map<T: SetDefault + Any + core::fmt::Debug>(
        &mut self,
        vaddr: VAddr,
        page: &PageCap<T>,
        untyped: &mut UntypedDescriptor,
        cpool: &mut CPoolDescriptor,
        perms: crate::capability::MapPermissions,
    ) -> Result<(), CapabilityErrors> {
        let mut pdpt_cap: PDPTCap = {
            let index = pml4_index(vaddr);

            if !self.read().read()[index].is_present() {
                let pdpt_cap = PDPTCap::retype_from(untyped)?;
                cpool.downgrade_free(pdpt_cap.clone())?;
                self.map_pdpt(index, &pdpt_cap)?;
            }

            let position = (0..cpool.size())
                .position(|i| {
                    let any = cpool.upgrade_any(i);
                    if let Some(any) = any {
                        if any.is::<PDPTCap>() {
                            let cap: PDPTCap = any.into();
                            let cap_desc = cap.read();
                            cap_desc.start_paddr()
                                == self.read().read()[index].get_address().to_paddr_global()
                        } else {
                            mem::drop(any);
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
                cpool.downgrade_free(pd_cap.clone())?;
                pdpt_cap.map_pd(index, &pd_cap)?;
            }

            let position = (0..cpool.size())
                .position(|i| {
                    let any = cpool.upgrade_any(i);
                    if let Some(any) = any {
                        if any.is::<PDCap>() {
                            let cap: PDCap = any.into();
                            let cap_desc = cap.read();
                            cap_desc.start_paddr()
                                == pdpt_cap.read().read()[index]
                                    .get_address()
                                    .to_paddr_global()
                        } else {
                            mem::drop(any);
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
                cpool.downgrade_free(pt_cap.clone())?;
                pd_cap.map_pt(index, &pt_cap)?;
            }

            let position = (0..cpool.size())
                .position(|i| {
                    let any = cpool.upgrade_any(i);
                    if let Some(any) = any {
                        if any.is::<PTCap>() {
                            let cap: PTCap = any.into();
                            let cap_desc = cap.read();
                            cap_desc.start_paddr()
                                == pd_cap.read().read()[index].get_address().to_paddr_global()
                        } else {
                            mem::drop(any);
                            false
                        }
                    } else {
                        false
                    }
                })
                .ok_or(CapabilityErrors::CapabilitySearchFailed)?;

            cpool.upgrade(position).unwrap()
        };

        pt_cap.map_page(pt_index(vaddr), page, perms)?;
        Ok(())
    }
}

impl PML4Descriptor {
    pub fn start_paddr(&self) -> PAddrGlobal {
        self.start_paddr
    }

    pub fn length(&self) -> usize {
        BASE_PAGE_LENGTH
    }

    fn page_object(&self) -> NonNull<PML4> {
        let addr: u64 = self.start_paddr.into();
        NonNull::new(addr as _).unwrap()
    }

    pub fn read(&self) -> &PML4 {
        unsafe { self.page_object().as_ref() }
    }

    fn write(&mut self) -> &mut PML4 {
        unsafe { self.page_object().as_mut() }
    }

    pub fn switch_to(&mut self) {
        use crate::arch::paging;

        unsafe {
            paging::utils::switch_to(self.start_paddr.to_paddr());
        }
    }
}
