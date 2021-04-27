use relic_abi::cap::CapabilityErrors;

use super::*;
use crate::{arch::globals::BASE_PAGE_LENGTH, util::boxed::Boxed};

#[derive(Debug)]
#[repr(C, align(4096))]
pub struct PTTable([PTEntry; 512]);

impl core::ops::Deref for PTTable {
    type Target = [PTEntry; 512];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl core::ops::DerefMut for PTTable {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Debug)]
pub struct L1 {
    pub page_data: Boxed<PTTable>,
    pub parent_pml4: Option<StoredCap>,
}

impl StoredCap {
    pub fn pt_retype_from(
        untyped: &mut UntypedMemory,
        cpool_to_store_in: &mut Cpool,
    ) -> Result<(StoredCap, usize), CapabilityErrors> {
        let mut result_index = 0;

        let cap = untyped.derive(|memory| {
            unsafe {
                core::ptr::write(memory, [PTEntry::empty(); 512]);
            }
            let boxed = unsafe { Boxed::new((memory as u64).into()) };

            let stored_index = cpool_to_store_in.get_free_index()?;
            let cap = cpool_to_store_in.write_to_if_empty(
                stored_index,
                Capability {
                    capability_data: CapabilityEnum::L1(L1 {
                        parent_pml4: None,
                        page_data: boxed,
                    }),
                    ..Default::default()
                },
            )?;

            result_index = stored_index;
            Ok(cap)
        })?;

        Ok((cap, result_index))
    }
}

impl L1 {
    pub fn start_paddr(&self) -> PAddrGlobal {
        self.page_data.paddr_global()
    }

    pub fn length(&self) -> usize {
        BASE_PAGE_LENGTH
    }
}

impl StoredCap {
    pub fn l1_map_raw_page(
        &self,
        index: usize,
        raw_page: &StoredCap,
    ) -> Result<(), CapabilityErrors> {
        let soon_to_be_second = self.borrow().next_paging_item.clone();

        self.l1_create_mut(|l1_write| {
            if l1_write.page_data[index].is_present() {
                return Err(CapabilityErrors::MemoryAlreadyMapped);
            }

            raw_page.raw_page_create_mut(|raw_page| {
                if raw_page.parent_pml4.is_some() {
                    return Err(CapabilityErrors::MemoryAlreadyMapped);
                }

                l1_write.page_data[index] = PTEntry::new(
                    raw_page.start_paddr().to_paddr(),
                    PTEntry::PRESENT | PTEntry::READ_WRITE | PTEntry::USERSPACE,
                );

                raw_page.parent_pml4 = l1_write.parent_pml4.clone();
                Ok(())
            })?;

            raw_page.borrow_mut().next_mem_item = soon_to_be_second.clone();
            raw_page.borrow_mut().prev_mem_item = Some(self.clone());
            if let Some(soon_to_be_sec_val) = soon_to_be_second {
                soon_to_be_sec_val.borrow_mut().prev_mem_item = Some(raw_page.clone());
            }

            Ok(())
        })?;

        self.borrow_mut().next_mem_item = Some(raw_page.clone());
        Ok(())
    }
}
