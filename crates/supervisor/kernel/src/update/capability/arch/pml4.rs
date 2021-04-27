use relic_abi::cap::CapabilityErrors;

use super::*;
use crate::{addr::VAddr, arch::globals::BASE_PAGE_LENGTH, util::boxed::Boxed};

#[derive(Debug)]
#[repr(C, align(4096))]
pub struct PML4Table([PML4Entry; 512]);

impl core::ops::Deref for PML4Table {
    type Target = [PML4Entry; 512];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl core::ops::DerefMut for PML4Table {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Debug)]
pub struct L4 {
    pub page_data: Boxed<PML4Table>,

    pub child_paging_item: Option<StoredCap>,
}

impl Capability {
    pub fn pml4_retype_from(
        untyped: &mut UntypedMemory,
        cpool_to_store_in: &mut Cpool,
    ) -> Result<(StoredCap, usize), CapabilityErrors> {
        let mut result_index = 0;

        let result = untyped.derive(|memory| {
            unsafe {
                core::ptr::write(memory, [PML4Entry::empty(); 512]);
            }
            let boxed = unsafe { Boxed::new((memory as u64).into()) };

            let stored_index = cpool_to_store_in.get_free_index()?;
            let cap = cpool_to_store_in.write_to_if_empty(
                stored_index,
                Capability {
                    capability_data: CapabilityEnum::L4(L4 {
                        page_data: boxed,
                        child_paging_item: None,
                    }),
                    ..Default::default()
                },
            )?;

            result_index = stored_index;
            Ok(cap)
        })?;

        Ok((result, result_index))
    }
}

impl L4 {
    pub fn start_paddr(&self) -> PAddrGlobal {
        self.page_data.paddr_global()
    }

    pub fn length(&self) -> usize {
        BASE_PAGE_LENGTH
    }
}

impl StoredCap {
    pub fn l4_map_l3(&self, index: usize, pdpt_page: &StoredCap) -> Result<(), CapabilityErrors> {
        self.l4_create_mut(|l4_write| {
            if l4_write.page_data[index].is_present() {
                return Err(CapabilityErrors::MemoryAlreadyMapped);
            }
            let soon_to_be_second = l4_write.child_paging_item.clone();

            pdpt_page.l3_create_mut(|pdpt_write| {
                if pdpt_write.next_paging_item.is_some() {
                    return Err(CapabilityErrors::MemoryAlreadyMapped);
                }

                l4_write.page_data[index] = PML4Entry::new(
                    pdpt_write.start_paddr().to_paddr(),
                    PML4Entry::PRESENT | PML4Entry::READ_WRITE | PML4Entry::USERSPACE,
                );

                pdpt_write.next_paging_item = soon_to_be_second.clone();
                pdpt_write.prev_paging_item = Some(self.clone());
                Ok(())
            })?;

            if let Some(soon_to_be_sec_val) = soon_to_be_second {
                *soon_to_be_sec_val.borrow_mut().get_prev_paging_item_mut() =
                    Some(pdpt_page.clone());
            }

            l4_write.child_paging_item = Some(pdpt_page.clone());
            Ok(())
        })
    }

    pub fn l4_map(
        &self,
        vaddr: VAddr,
        raw_page: &StoredCap,
        untyped: &mut UntypedMemory,
        cpool: &mut Cpool,
    ) -> Result<(), CapabilityErrors> {
        let pdpt_cap = {
            let pml4_index = pml4_index(vaddr);
            let mut l4_address = PAddrGlobal::new(0);

            if !self.l4_create(|l4_read| {
                l4_address = l4_read.page_data[pml4_index]
                    .get_address()
                    .to_paddr_global();
                Ok(l4_read.page_data[pml4_index].is_present())
            })? {
                let pdpt = StoredCap::pdpt_retype_from(untyped, cpool)?;
                self.l4_map_l3(pml4_index, &pdpt.0)?;
            }

            let l3_cap_index = (0..cpool.size())
                .position(|i| {
                    let current: StoredCap = (&cpool.data.unsafe_data[i]).into();
                    current
                        .l3_create(|l3| Ok(l3.start_paddr() == l4_address))
                        .unwrap_or(false)
                })
                .ok_or(CapabilityErrors::CapabilitySearchFailed)?;

            cpool.lookup_index_unsafe(l3_cap_index)
        };

        let pd_cap = {
            let pdpt_index = pdpt_index(vaddr);
            let mut l3_address = PAddrGlobal::new(0);

            if !pdpt_cap.l3_create(|l3_read| {
                l3_address = l3_read.page_data[pdpt_index]
                    .get_address()
                    .to_paddr_global();
                Ok(l3_read.page_data[pdpt_index].is_present())
            })? {
                let pd = StoredCap::pd_retype_from(untyped, cpool)?;
                pdpt_cap.l3_map_l2(pdpt_index, &pd.0)?;
            }

            let l2_cap_index = (0..cpool.size())
                .position(|i| {
                    let current: StoredCap = (&cpool.data.unsafe_data[i]).into();
                    current
                        .l2_create(|l2| Ok(l2.start_paddr() == l3_address))
                        .unwrap_or(false)
                })
                .ok_or(CapabilityErrors::CapabilitySearchFailed)?;

            cpool.lookup_index_unsafe(l2_cap_index)
        };

        let pt_cap = {
            let pd_index = pd_index(vaddr);
            let mut l2_address = PAddrGlobal::new(0);

            if !pd_cap.l2_create(|l2_read| {
                l2_address = l2_read.page_data[pd_index].get_address().to_paddr_global();
                Ok(l2_read.page_data[pd_index].is_present())
            })? {
                let pt = StoredCap::pt_retype_from(untyped, cpool)?;
                pd_cap.l2_map_l1(pd_index, &pt.0)?;
            }

            let l1_cap_index = (0..cpool.size())
                .position(|i| {
                    let current: StoredCap = (&cpool.data.unsafe_data[i]).into();
                    current
                        .l1_create(|l1| Ok(l1.start_paddr() == l2_address))
                        .unwrap_or(false)
                })
                .ok_or(CapabilityErrors::CapabilitySearchFailed)?;

            cpool.lookup_index_unsafe(l1_cap_index)
        };

        pt_cap.l1_map_raw_page(pt_index(vaddr), raw_page)
    }
}
