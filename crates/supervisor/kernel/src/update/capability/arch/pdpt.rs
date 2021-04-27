use relic_abi::cap::CapabilityErrors;

use super::*;
use crate::{arch::globals::BASE_PAGE_LENGTH, util::boxed::Boxed};

#[derive(Debug)]
#[repr(C, align(4096))]
pub struct PDPTTable([PDPTEntry; 512]);

impl core::ops::Deref for PDPTTable {
    type Target = [PDPTEntry; 512];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl core::ops::DerefMut for PDPTTable {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Debug)]
pub struct L3 {
    pub page_data: Boxed<PDPTTable>,

    pub child_paging_item: Option<StoredCap>,
    pub next_paging_item: Option<StoredCap>,
    pub prev_paging_item: Option<StoredCap>,
}

impl StoredCap {
    pub fn pdpt_retype_from(
        untyped: &mut UntypedMemory,
        cpool_to_store_in: &mut Cpool,
    ) -> Result<(StoredCap, usize), CapabilityErrors> {
        let mut result_index = 0;

        let result = untyped.derive(|memory| {
            unsafe {
                core::ptr::write(memory, PDPTTable([PDPTEntry::empty(); 512]));
            }
            let boxed = unsafe { Boxed::new((memory as u64).into()) };

            let stored_index = cpool_to_store_in.get_free_index()?;
            let cap = cpool_to_store_in.write_to_if_empty(
                stored_index,
                Capability {
                    capability_data: CapabilityEnum::L3(L3 {
                        page_data: boxed,
                        child_paging_item: None,
                        next_paging_item: None,
                        prev_paging_item: None,
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

impl L3 {
    pub fn start_paddr(&self) -> PAddrGlobal {
        self.page_data.paddr_global()
    }

    pub fn length(&self) -> usize {
        BASE_PAGE_LENGTH
    }
}

impl StoredCap {
    pub fn l3_map_l2(&self, index: usize, pd_page: &StoredCap) -> Result<(), CapabilityErrors> {
        self.l3_create_mut(|l3_write| {
            if l3_write.page_data[index].is_present() {
                return Err(CapabilityErrors::MemoryAlreadyMapped);
            }
            let soon_to_be_second = l3_write.child_paging_item.clone();

            pd_page.l2_create_mut(|pd_page_data| {
                if pd_page_data.next_paging_item.is_some() {
                    return Err(CapabilityErrors::MemoryAlreadyMapped);
                }

                l3_write.page_data[index] = PDPTEntry::new(
                    pd_page_data.start_paddr().to_paddr(),
                    PDPTEntry::PRESENT | PDPTEntry::READ_WRITE | PDPTEntry::USERSPACE,
                );

                pd_page_data.next_paging_item = soon_to_be_second.clone();
                pd_page_data.prev_paging_item = Some(self.clone());
                Ok(())
            })?;

            if let Some(soon_to_be_sec_val) = soon_to_be_second {
                *soon_to_be_sec_val.borrow_mut().get_prev_paging_item_mut() = Some(pd_page.clone());
            }

            l3_write.child_paging_item = Some(pd_page.clone());
            Ok(())
        })
    }
}
