use super::*;
mod pml4;
mod raw_page;

pub use pml4::*;
pub use raw_page::*;

use crate::arch::globals::BASE_PAGE_LENGTH;
use crate::util::boxed::Boxed;

macro_rules! paging_cap_impl {
    ($paging: ty, $inner: ty, with_child) => {
        paste! {
            #[derive(Debug)]
            pub struct $paging {
                pub page_data: Boxed<[<$inner Table>]>,

                pub child_paging_item: Option<StoredCap>,
                pub next_paging_item: Option<StoredCap>,
                pub prev_paging_item: Option<StoredCap>,
            }

            impl $paging {
                pub fn new(boxed: Boxed<[<$inner Table>]>) -> Self {
                    Self {
                        page_data: boxed,
                        child_paging_item: None,
                        next_paging_item: None,
                        prev_paging_item: None,
                    }
                }
            }
        }
    };
    ($paging: ty, $inner: ty, $child: ty) => {
        paste! {
            #[derive(Debug)]
            #[repr(C, align(4096))]
            pub struct [< $inner Table >]([ [< $inner Entry >]; 512]);

            impl core::ops::Deref for  [< $inner Table >] {
                type Target = [ [< $inner Entry >] ; 512];

                fn deref(&self) -> &Self::Target {
                    &self.0
                }
            }

            impl core::ops::DerefMut for  [< $inner Table >] {
                fn deref_mut(&mut self) -> &mut Self::Target {
                    &mut self.0
                }
            }

            impl StoredCap {
                pub fn [< $inner:lower _retype_from >](
                    untyped: &mut UntypedMemory,
                    cpool_to_store_in: &mut Cpool,
                ) -> Result<(StoredCap, usize), CapabilityErrors> {
                    let mut result_index = 0;

                    let result = untyped.derive(|memory: *mut [< $inner Table >]| {
                        unsafe {
                            core::ptr::write(memory, [< $inner Table >]([[< $inner Entry >]::empty(); 512]));
                        }
                        let boxed = unsafe { Boxed::new((memory as u64).into()) };

                        let stored_index = cpool_to_store_in.get_free_index()?;
                        let cap = cpool_to_store_in.write_to_if_empty(
                            stored_index,
                            Capability {
                                capability_data: CapabilityEnum::$paging($paging::new(boxed)),
                                ..Default::default()
                            },
                        )?;

                        result_index = stored_index;
                        Ok(cap)
                    })?;

                    Ok((result, result_index))
                }
            }

            impl $paging {
                pub fn start_paddr(&self) -> PAddrGlobal {
                    self.page_data.paddr_global()
                }

                pub fn length(&self) -> usize {
                    BASE_PAGE_LENGTH
                }
            }

            impl StoredCap {
                pub fn [< $paging:lower _map_ $child:snake >](&self, index: usize, child: &StoredCap)
                    -> Result<(), CapabilityErrors> {
                        self.[<$paging:lower _create_mut>](|self_write| {
                            if self_write.page_data[index].is_present() {
                                return Err(CapabilityErrors::MemoryAlreadyMapped);
                            }
                            let soon_to_be_second = self_write.child_paging_item.clone();

                            child.[<$child:snake _create_mut>](|child_write| {
                                if child_write.next_paging_item.is_some() {
                                    return Err(CapabilityErrors::MemoryAlreadyMapped);
                                }

                                self_write.page_data[index] = [< $inner Entry >]::new(
                                    child_write.start_paddr().to_paddr(),
                                    [< $inner Entry >]::PRESENT | [< $inner Entry >]::READ_WRITE | [< $inner Entry >]::USERSPACE,
                                );

                                child_write.next_paging_item = soon_to_be_second.clone();
                                child_write.prev_paging_item = Some(self.clone());
                                Ok(())
                            })?;

                            if let Some(soon_to_be_sec_val) = soon_to_be_second {
                                *soon_to_be_sec_val.borrow_mut().get_prev_paging_item_mut() =
                                    Some(child.clone());
                            }

                            self_write.child_paging_item = Some(child.clone());
                            Ok(())
                        })
                }
            }
        }
    };
}

paging_cap_impl!(L1, PT, with_child);
paging_cap_impl!(L2, PD, with_child);
paging_cap_impl!(L3, PDPT, with_child);

paging_cap_impl!(L4, PML4, L3);
paging_cap_impl!(L3, PDPT, L2);
paging_cap_impl!(L2, PD, L1);
paging_cap_impl!(L1, PT, RawPage);
