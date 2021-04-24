use relic_abi::cap::CapabilityErrors;

use super::*;
use crate::{arch::globals::BASE_PAGE_LENGTH, util::boxed::Boxed};

#[derive(Debug)]
pub struct L3 {
    pub page_data: Boxed<[PDPTEntry; 512]>,
    pub parent_pml4: Option<UnsafeRef<Capability>>,
}

impl Capability {
    pub fn pdpt_retype_from(
        untyped: &mut UntypedMemory,
        cpool_to_store_in: &mut Cpool,
    ) -> Result<(UnsafeRef<Capability>, usize), CapabilityErrors> {
        let mut result_index = 0;

        let result = untyped.derive(|memory| {
            unsafe {
                core::ptr::write(memory, [PDPTEntry::empty(); 512]);
            }
            let boxed = unsafe { Boxed::new((memory as u64).into()) };

            let stored_index = cpool_to_store_in.get_free_index()?;
            let cap = cpool_to_store_in.write_to_if_empty(
                stored_index,
                Capability {
                    mem_tree_link: LinkedListLink::new(),
                    paging_tree_link: LinkedListLink::new(),
                    capability_data: RefCell::new(CapabilityEnum::L3(L3 {
                        parent_pml4: None,
                        page_data: boxed,
                    })),
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

impl Capability {
    pub fn l3_map_l2(&self, index: usize, pd_page: &Capability) -> Result<(), CapabilityErrors> {
        self.l3_create_mut(|l3_write| {
            if l3_write.page_data[index].is_present() {
                return Err(CapabilityErrors::MemoryAlreadyMapped);
            }

            pd_page.l2_create_mut(|pd_page| {
                if pd_page.parent_pml4.is_some() {
                    return Err(CapabilityErrors::MemoryAlreadyMapped);
                }

                l3_write.page_data[index] = PDPTEntry::new(
                    pd_page.start_paddr().to_paddr(),
                    PDPTEntry::PRESENT | PDPTEntry::READ_WRITE | PDPTEntry::USERSPACE,
                );

                pd_page.parent_pml4 = l3_write.parent_pml4.clone();
                Ok(())
            })?;

            // Insert the new entry in the mem tree.
            let refcell = unsafe { UnsafeRef::from_raw(pd_page) };
            let pml4 = l3_write.parent_pml4.clone().unwrap();
            pml4.l4_create_mut(|l4| {
                let ll = &mut l4.children;
                let mut cursor = unsafe { ll.cursor_mut_from_ptr(self) };
                cursor.insert_after(refcell);

                Ok(())
            })
        })
    }
}
