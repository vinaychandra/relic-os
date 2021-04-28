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
