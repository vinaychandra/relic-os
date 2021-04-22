use relic_abi::cap::CapabilityErrors;

use crate::{
    addr::PAddrGlobal,
    arch::{globals::BASE_PAGE_LENGTH, paging::table::*},
    capability::MapPermissions,
    update::capability::{arch::ArchCap, CPoolInner, CPoolRead, CapRcBoxed, Capability},
    util::boxed::{Boxed, RcRefCellBoxed, RcRefCellBoxedInner},
};

/// Page length used in current kernel. This is `BASE_PAGE_LENGTH` in x86_64.
pub const PAGE_LENGTH: usize = BASE_PAGE_LENGTH;

/// Page descriptor.
pub struct RawPageRead<'a> {
    page_data: &'a Boxed<[u8; 4096]>,
    mapped_page: &'a Option<CapRcBoxed>,
}

pub struct RawPageWrite<'a> {
    page_data: &'a mut Boxed<[u8; 4096]>,
    mapped_page: &'a mut Option<CapRcBoxed>,
}

macro_rules! page_cap_impl {
    ($name: ident) => {
        paste! {
            pub struct [<$name DescriptorWrite>]<'a> {
                page_data: &'a mut Boxed<[ [<$name Entry>] ; 512]>,
                mapped_page: &'a mut Option<CapRcBoxed>,
            }

            pub struct  [<$name DescriptorRead>]<'a> {
                page_data: &'a Boxed<[ [<$name Entry>]; 512]>,
                mapped_page: &'a Option<CapRcBoxed>,
            }

            impl<'a> [<$name DescriptorRead>]<'a> {
                pub fn start_paddr(&self) -> PAddrGlobal {
                    self.page_data.paddr_global()
                }

                pub fn length(&self) -> usize {
                    BASE_PAGE_LENGTH
                }
            }

            impl Capability {
                pub fn [<$name:lower _create>](&self) -> Option< [<$name DescriptorRead>] <'_>> {
                    if let Capability::Arch(ArchCap::$name {
                        page_data,
                        mapped_page,
                        ..
                    }) = self
                    {
                        Some([<$name DescriptorRead>] {
                            mapped_page,
                            page_data,
                        })
                    } else {
                        None
                    }
                }

                pub fn [<$name:lower _create_mut>](&mut self) -> Option< [<$name DescriptorWrite>] <'_>> {
                    if let Capability::Arch(ArchCap::$name {
                        page_data,
                        mapped_page,
                        ..
                    }) = self
                    {
                        Some([<$name DescriptorWrite>] {
                            mapped_page,
                            page_data,
                        })
                    } else {
                        None
                    }
                }

                pub fn [<$name:lower _retype_from>](
                    untyped: &CapRcBoxed,
                    cpool_to_store_in: &CPoolRead,
                ) -> Result<(CapRcBoxed, usize), CapabilityErrors> {
                    let untyped_borrow = &mut *untyped.borrow_mut();
                    let mut untyped_data = untyped_borrow
                        .untyped_create_mut()
                        .ok_or(CapabilityErrors::CapabilityMismatch)?;

                    let mut result_index = 0;
                    let mut boxed2 = None;

                    untyped_data.derive(|memory, next_child| {
                        unsafe {
                            core::ptr::write(memory, [ [<$name Entry>]::empty(); 512]);
                        }
                        let boxed = unsafe { Boxed::new((memory as u64).into()) };

                        let result = cpool_to_store_in.store_free(Capability::Arch(ArchCap::$name {
                            next: next_child.clone(),
                            mapped_page: None,
                            prev: Some(untyped.clone()),
                            page_data: boxed,
                        }))?;

                        if let Some(a) = &next_child {
                            *a.borrow_mut().get_prev_mut() = Some(result.1.clone());
                        }

                        boxed2 = Some(result.1.clone());
                        result_index = result.0;
                        Ok(result.1)
                    })?;

                    Ok((boxed2.unwrap(), result_index))
                }
            }
        }
    };
}

page_cap_impl!(PML4);
page_cap_impl!(PDPT);
page_cap_impl!(PD);
page_cap_impl!(PT);

// impl RcRefCellBoxed<PDDescriptor> {
//     pub fn map_pd(
//         &mut self,
//         index: usize,
//         sub: &mut PTDescriptor,
//         perms: MapPermissions,
//     ) -> Result<(), CapabilityErrors> {
//         let current_data = &mut self.borrow_mut().page_data;
//         if current_data[index].is_present() {
//             return Err(CapabilityErrors::CapabilityAlreadyOccupied);
//         }

//         let mut flags = PDEntry::PRESENT | PDEntry::USERSPACE;
//         if perms.contains(crate::capability::MapPermissions::WRITE) {
//             flags |= PDEntry::READ_WRITE;
//         }
//         if !perms.contains(crate::capability::MapPermissions::EXECUTE) {
//             flags |= PDEntry::EXECUTE_DISABLE;
//         }

//         Ok(())
//     }
// }
