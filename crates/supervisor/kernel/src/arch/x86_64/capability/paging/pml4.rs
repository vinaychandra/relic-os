use relic_abi::cap::CapabilityErrors;

use super::*;
use crate::{addr::VAddr, arch::paging::utils, util::boxed::Boxed};

#[derive(Debug)]
pub struct L4 {
    pub page_data: Boxed<PML4Table>,
    pub child_paging_item: Option<StoredCap>,
    pub linked_task: Option<StoredCap>,
}

impl L4 {
    #[allow(unused_mut)]
    pub fn new(mut boxed: Boxed<PML4Table>) -> Self {
        #[cfg(not(test))]
        unsafe {
            let current_page_table_paddr: u64 = utils::cr3().into();
            let current_page_table: &PML4 = &*(current_page_table_paddr as *const _);
            boxed[510] = current_page_table[510];
            boxed[511] = current_page_table[511];
        }

        Self {
            linked_task: None,
            page_data: boxed,
            child_paging_item: None,
        }
    }

    pub fn switch_to(&mut self) {
        use crate::arch::paging;

        unsafe {
            paging::utils::switch_to(self.page_data.paddr_global().to_paddr());
        }
    }
}

impl CapAccessorMut<'_, L4> {
    /**
    Map the given raw page in the provided L4 table at the given virtual address.
    This will create any required intermediate pages in the provided untyped memory.
    In this case, it can create upto 3 more levels of paging: each for PDPT, PD and PT.

    The map will search for other permissions in `search_cpool` whereas it will store newly
    created capabilities in `store_cpool`. If `search_cpool` is empty, `store_cpool` will be used.
    */
    pub fn l4_map(
        &mut self,
        vaddr: VAddr,
        raw_page: &StoredCap,
        untyped: &mut CapAccessorMut<'_, UntypedMemory>,
        store_cpool: &mut CapAccessorMut<'_, Cpool>,
        search_cpool: Option<&mut CapAccessorMut<'_, Cpool>>,
        perms: MapPermissions,
    ) -> Result<(), CapabilityErrors> {
        // get size of raw_page
        let page_type = {
            match &raw_page.borrow().capability_data {
                CapabilityEnum::BasePage(_) => 4,
                CapabilityEnum::LargePage(_) => 3,
                CapabilityEnum::HugePage(_) => 2,
                _ => return Err(CapabilityErrors::CapabilityMismatch),
            }
        };
        let pml4_index = pml4_index(vaddr);
        let pdpt_index = pdpt_index(vaddr);
        let pd_index = pd_index(vaddr);
        let pt_index = pt_index(vaddr);

        // L4
        let mut l4_address = self.page_data[pml4_index].get_address().to_paddr_global();
        if !(self.page_data[pml4_index].is_present()) {
            let pdpt = StoredCap::pdpt_retype_from(untyped, store_cpool)?;
            let mut pdpt_l3 = pdpt.0.as_l3_mut().unwrap();
            l4_address = pdpt_l3.page_data.paddr_global();

            self.l4_map_l3(pml4_index, &mut pdpt_l3, None)?;
        }
        let pdpt = match &search_cpool {
            Some(search_cpool) => search_cpool
                .search_fn(|cap| {
                    cap.as_l3_mut()
                        .map(|v| v.start_paddr() == l4_address)
                        .unwrap_or(false)
                })
                .or_else(|_| {
                    store_cpool.search_fn(|cap| {
                        cap.as_l3_mut()
                            .map(|v| v.start_paddr() == l4_address)
                            .unwrap_or(false)
                    })
                })?,
            None => store_cpool.search_fn(|cap| {
                cap.as_l3_mut()
                    .map(|v| v.start_paddr() == l4_address)
                    .unwrap_or(false)
            })?,
        };
        let mut pdpt_cap = pdpt.as_l3_mut().unwrap();

        // L3
        if page_type == 2 {
            if pd_index != 0 || pt_index != 0 {
                return Err(CapabilityErrors::MemoryAlignmentFailure);
            }

            let mut target_perms = PDPTEntry::empty();
            if perms.contains(MapPermissions::WRITE) {
                target_perms |= PDPTEntry::READ_WRITE;
            }
            if !perms.contains(MapPermissions::EXECUTE) {
                target_perms |= PDPTEntry::EXECUTE_DISABLE;
            }
            if perms.contains(MapPermissions::CACHE_DISABLE) {
                target_perms |= PDPTEntry::CACHE_DISABLE;
            }
            return pdpt_cap.l3_map_huge_page(
                pdpt_index,
                &mut raw_page.as_huge_page_mut().unwrap(),
                Some(target_perms),
            );
        }

        let mut l3_address = pdpt_cap.page_data[pdpt_index]
            .get_address()
            .to_paddr_global();

        if !(pdpt_cap.page_data[pdpt_index].is_present()) {
            let pd = StoredCap::pd_retype_from(untyped, store_cpool)?;
            let mut pd_l2 = pd.0.as_l2_mut().unwrap();
            l3_address = pd_l2.page_data.paddr_global();

            pdpt_cap.l3_map_l2(pdpt_index, &mut pd_l2, None)?;
        }

        let pd = match &search_cpool {
            Some(search_cpool) => search_cpool
                .search_fn(|cap| {
                    cap.as_l2_mut()
                        .map(|l2| l2.start_paddr() == l3_address)
                        .unwrap_or(false)
                })
                .or_else(|_| {
                    store_cpool.search_fn(|cap| {
                        cap.as_l2_mut()
                            .map(|l2| l2.start_paddr() == l3_address)
                            .unwrap_or(false)
                    })
                })?,
            None => store_cpool.search_fn(|cap| {
                cap.as_l2_mut()
                    .map(|l2| l2.start_paddr() == l3_address)
                    .unwrap_or(false)
            })?,
        };

        let mut pd_cap = pd.as_l2_mut().unwrap();

        // L2
        if page_type == 3 {
            if pt_index != 0 {
                return Err(CapabilityErrors::MemoryAlignmentFailure);
            }

            let mut target_perms = PDEntry::empty();
            if perms.contains(MapPermissions::WRITE) {
                target_perms |= PDEntry::READ_WRITE;
            }
            if !perms.contains(MapPermissions::EXECUTE) {
                target_perms |= PDEntry::EXECUTE_DISABLE;
            }
            if perms.contains(MapPermissions::CACHE_DISABLE) {
                target_perms |= PDEntry::CACHE_DISABLE;
            }
            return pd_cap.l2_map_large_page(
                pd_index,
                &mut raw_page.as_large_page_mut().unwrap(),
                Some(target_perms),
            );
        }

        let mut l2_address = pd_cap.page_data[pd_index].get_address().to_paddr_global();

        if !(pd_cap.page_data[pd_index].is_present()) {
            let pt = StoredCap::pt_retype_from(untyped, store_cpool)?;
            let mut pt_l1 = pt.0.as_l1_mut().unwrap();
            l2_address = pt_l1.page_data.paddr_global();

            pd_cap.l2_map_l1(pd_index, &mut pt_l1, None)?;
        }

        let pt = match &search_cpool {
            Some(search_cpool) => search_cpool
                .search_fn(|cap| {
                    cap.as_l1_mut()
                        .map(|l1| l1.start_paddr() == l2_address)
                        .unwrap_or(false)
                })
                .or_else(|_| {
                    store_cpool.search_fn(|cap| {
                        cap.as_l1_mut()
                            .map(|l1| l1.start_paddr() == l2_address)
                            .unwrap_or(false)
                    })
                })?,
            None => store_cpool.search_fn(|cap| {
                cap.as_l1_mut()
                    .map(|l1| l1.start_paddr() == l2_address)
                    .unwrap_or(false)
            })?,
        };

        let mut pt_cap = pt.as_l1_mut().unwrap();

        // L1
        let mut target_perms = PTEntry::empty();
        if perms.contains(MapPermissions::WRITE) {
            target_perms |= PTEntry::READ_WRITE;
        }
        if !perms.contains(MapPermissions::EXECUTE) {
            target_perms |= PTEntry::EXECUTE_DISABLE;
        }
        if perms.contains(MapPermissions::CACHE_DISABLE) {
            target_perms |= PTEntry::CACHE_DISABLE;
        }
        pt_cap.l1_map_base_page(
            pt_index,
            &mut raw_page.as_base_page_mut().unwrap(),
            Some(target_perms),
        )
    }
}
