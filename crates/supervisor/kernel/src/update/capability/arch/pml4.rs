use relic_abi::cap::CapabilityErrors;

use super::*;
use crate::{addr::VAddr, util::boxed::Boxed};

#[derive(Debug)]
pub struct L4 {
    pub page_data: Boxed<PML4Table>,
    pub child_paging_item: Option<StoredCap>,
}

impl L4 {
    pub fn new(boxed: Boxed<PML4Table>) -> Self {
        Self {
            page_data: boxed,
            child_paging_item: None,
        }
    }
}

impl StoredCap {
    pub fn l4_map(
        &self,
        vaddr: VAddr,
        raw_page: &StoredCap,
        untyped: &mut UntypedMemory,
        cpool: &mut Cpool,
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

        let pdpt_cap = {
            let mut l4_address = PAddrGlobal::new(0);

            if !self.l4_create(|l4_read| {
                l4_address = l4_read.page_data[pml4_index]
                    .get_address()
                    .to_paddr_global();
                Ok(l4_read.page_data[pml4_index].is_present())
            })? {
                let pdpt = StoredCap::pdpt_retype_from(untyped, cpool)?;
                l4_address = pdpt.0.l3_create(|a| Ok(a.page_data.paddr_global()))?;
                self.l4_map_l3(pml4_index, &pdpt.0)?;
            }

            cpool.search_fn(|cap| {
                cap.l3_create(|l3| Ok(l3.start_paddr() == l4_address))
                    .unwrap_or(false)
            })?
        };

        let pd_cap = {
            let mut l3_address = PAddrGlobal::new(0);

            if page_type == 2 {
                if pd_index != 0 || pt_index != 0 {
                    return Err(CapabilityErrors::MemoryAlignmentFailure);
                }
                return pdpt_cap.l3_map_huge_page(pdpt_index, raw_page);
            }

            if !pdpt_cap.l3_create(|l3_read| {
                l3_address = l3_read.page_data[pdpt_index]
                    .get_address()
                    .to_paddr_global();
                Ok(l3_read.page_data[pdpt_index].is_present())
            })? {
                let pd = StoredCap::pd_retype_from(untyped, cpool)?;
                l3_address = pd.0.l2_create(|a| Ok(a.page_data.paddr_global()))?;
                pdpt_cap.l3_map_l2(pdpt_index, &pd.0)?;
            }

            cpool.search_fn(|cap| {
                cap.l2_create(|l2| Ok(l2.start_paddr() == l3_address))
                    .unwrap_or(false)
            })?
        };

        let pt_cap = {
            let mut l2_address = PAddrGlobal::new(0);

            if page_type == 3 {
                if pt_index != 0 {
                    return Err(CapabilityErrors::MemoryAlignmentFailure);
                }
                return pd_cap.l2_map_large_page(pd_index, raw_page);
            }

            if !pd_cap.l2_create(|l2_read| {
                l2_address = l2_read.page_data[pd_index].get_address().to_paddr_global();
                Ok(l2_read.page_data[pd_index].is_present())
            })? {
                let pt = StoredCap::pt_retype_from(untyped, cpool)?;
                l2_address = pt.0.l1_create(|a| Ok(a.page_data.paddr_global()))?;
                pd_cap.l2_map_l1(pd_index, &pt.0)?;
            }

            cpool.search_fn(|cap| {
                cap.l1_create(|l1| Ok(l1.start_paddr() == l2_address))
                    .unwrap_or(false)
            })?
        };

        pt_cap.l1_map_base_page(pt_index, raw_page)
    }
}
