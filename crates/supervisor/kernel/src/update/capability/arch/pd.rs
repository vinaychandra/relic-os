use relic_abi::cap::CapabilityErrors;

use super::*;
use crate::{arch::globals::BASE_PAGE_LENGTH, util::boxed::Boxed};

#[derive(Debug)]
#[repr(C, align(4096))]
pub struct PDTable([PDEntry; 512]);

impl core::ops::Deref for PDTable {
    type Target = [PDEntry; 512];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl core::ops::DerefMut for PDTable {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Debug)]
pub struct L2 {
    pub page_data: Boxed<PDTable>,
    pub parent_pml4: Option<StoredCap>,
}

impl StoredCap {
    pub fn pd_retype_from(
        untyped: &mut UntypedMemory,
        cpool_to_store_in: &mut Cpool,
    ) -> Result<(StoredCap, usize), CapabilityErrors> {
        let mut result_index = 0;

        let cap = untyped.derive(|memory| {
            unsafe {
                core::ptr::write(memory, PDTable([PDEntry::empty(); 512]));
            }
            let boxed = unsafe { Boxed::new((memory as u64).into()) };

            let stored_index = cpool_to_store_in.get_free_index()?;
            let cap = cpool_to_store_in.write_to_if_empty(
                stored_index,
                Capability {
                    capability_data: CapabilityEnum::L2(L2 {
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

impl L2 {
    pub fn start_paddr(&self) -> PAddrGlobal {
        self.page_data.paddr_global()
    }

    pub fn length(&self) -> usize {
        BASE_PAGE_LENGTH
    }
}

impl StoredCap {
    pub fn l2_map_l1(&self, index: usize, pt_page: &StoredCap) -> Result<(), CapabilityErrors> {
        let soon_to_be_second = self.borrow().next_paging_item.clone();

        self.l2_create_mut(|l2_write| {
            if l2_write.page_data[index].is_present() {
                return Err(CapabilityErrors::MemoryAlreadyMapped);
            }

            pt_page.l1_create_mut(|pt_page_data| {
                if pt_page_data.parent_pml4.is_some() {
                    return Err(CapabilityErrors::MemoryAlreadyMapped);
                }

                l2_write.page_data[index] = PDEntry::new(
                    pt_page_data.start_paddr().to_paddr(),
                    PDEntry::PRESENT | PDEntry::READ_WRITE | PDEntry::USERSPACE,
                );

                pt_page_data.parent_pml4 = l2_write.parent_pml4.clone();
                Ok(())
            })?;

            pt_page.borrow_mut().next_paging_item = soon_to_be_second.clone();
            pt_page.borrow_mut().prev_paging_item = Some(self.clone());
            if let Some(soon_to_be_sec_val) = soon_to_be_second {
                soon_to_be_sec_val.borrow_mut().prev_paging_item = Some(pt_page.clone());
            }

            Ok(())
        })?;

        self.borrow_mut().next_paging_item = Some(pt_page.clone());
        Ok(())
    }
}
