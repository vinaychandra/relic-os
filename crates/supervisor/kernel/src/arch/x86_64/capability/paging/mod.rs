mod pml4;
mod raw_page;

use relic_abi::cap::CapabilityErrors;

pub use pml4::*;
pub use raw_page::*;

use crate::{
    addr::PAddrGlobal,
    arch::{globals::BASE_PAGE_LENGTH, paging::table::*},
    capability::*,
    util::boxed::Boxed,
};

macro_rules! paging_cap_impl {
    ($paging: ty, $inner: ty, $child: ty, include_child_structs) => {
        paste! {
            #[derive(Debug)]
            pub struct $paging {
                pub page_data: Boxed<[<$inner Table>]>,

                pub child_paging_item: Option<StoredCap>,
                pub next_paging_item: Option<StoredCap>,
                pub prev_paging_item: Option<StoredCap>,
            }

            impl $paging {
                /**
                Create a new paging capability.
                */
                pub const fn new(boxed: Boxed<[<$inner Table>]>) -> Self {
                    Self {
                        page_data: boxed,
                        child_paging_item: None,
                        next_paging_item: None,
                        prev_paging_item: None,
                    }
                }
            }

            paging_cap_impl!($paging, $inner, $child);
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
                /**
                Create a page capability from untyped memory. This will store the created cap
                in the provided cpool. The function returns the [`StoredCap`] pointing
                to the created capability and an index in the cpool where this is created.
                */
                pub fn [< $inner:lower _retype_from >](
                    untyped: &mut CapAccessorMut<'_, UntypedMemory>,
                    cpool_to_store_in: &mut Cpool,
                ) -> Result<(StoredCap, usize), CapabilityErrors> {
                    let mut result_index = 0;

                    let result = untyped.derive(
                        Some(core::mem::size_of::<[< $inner Table >]>()),
                        false,
                        |memory: *mut [< $inner Table >]| {
                            unsafe {
                                core::ptr::write_bytes(memory as *mut u8, 0, 4096);
                            }
                            let boxed = unsafe { Boxed::new((memory as u64).into()) };

                            let stored_index = cpool_to_store_in.get_free_index()?;
                            let capability_data = CapabilityEnum::$paging($paging::new(boxed));
                            let cap = cpool_to_store_in.write_to_if_empty(
                                stored_index,
                                Capability {
                                    capability_data,
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
        }

        paging_cap_impl!($paging, $inner, $child, map_fn);
    };
    ($paging: ty, $inner: ty, $child: ty, map_fn $(, $extra_flags:tt)?) => {
        paste! {
            impl CapAccessorMut<'_, $paging> {
                pub fn [< $paging:lower _map_ $child:snake >](
                    &mut self,
                    index: usize,
                    child: &mut CapAccessorMut<'_, $child>,
                    permissions: Option<[< $inner Entry >]>,
                )
                    -> Result<(), CapabilityErrors> {

                        if self.page_data[index].is_present() {
                            return Err(CapabilityErrors::MemoryAlreadyMapped);
                        }
                        let soon_to_be_second = self.child_paging_item.clone();

                        if child.next_paging_item.is_some() {
                            return Err(CapabilityErrors::MemoryAlreadyMapped);
                        }

                        if let Some(perms) = permissions {
                            self.page_data[index] = [< $inner Entry >]::new(
                                child.start_paddr().to_paddr(),
                                [< $inner Entry >]::PRESENT | [< $inner Entry >]::USERSPACE | perms
                                $( | [< $inner Entry >]::$extra_flags )?,
                            );
                        } else {
                            self.page_data[index] = [< $inner Entry >]::new(
                                child.start_paddr().to_paddr(),
                                [< $inner Entry >]::PRESENT | [< $inner Entry >]::USERSPACE | [< $inner Entry >]::READ_WRITE
                                $( | [< $inner Entry >]::$extra_flags )?,
                            );
                        }

                        child.next_paging_item = soon_to_be_second.clone();
                        child.prev_paging_item = Some(self.cap().clone());

                        if let Some(soon_to_be_sec_val) = soon_to_be_second {
                            *soon_to_be_sec_val.borrow_mut().get_prev_paging_item_mut() =
                                Some(child.cap().clone());
                        }

                        self.child_paging_item = Some(child.cap().clone());
                        Ok(())
                }
            }
        }
    };
}

paging_cap_impl!(L1, PT, BasePage, include_child_structs);
paging_cap_impl!(L2, PD, L1, include_child_structs);
paging_cap_impl!(L3, PDPT, L2, include_child_structs);
paging_cap_impl!(L4, PML4, L3);

paging_cap_impl!(L2, PD, LargePage, map_fn, LARGE_PAGE);
paging_cap_impl!(L3, PDPT, HugePage, map_fn, HUGE_PAGE);

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, mem::MaybeUninit};

    use crate::util::unsafe_ref::UnsafeRef;

    use super::*;

    #[test]
    fn test_paging() {
        let raw_memory: Box<MaybeUninit<[u8; 0x20_0000 * 5]>> = Box::new_uninit();
        let raw_addr = Box::into_raw(raw_memory) as u64;
        let addr = PAddrGlobal::new(raw_addr);

        let untyped_memory = unsafe { UntypedMemory::bootstrap(addr, 0x20_0000 * 5, false) };
        const NONE_INNER: RefCell<Capability> = RefCell::new(Capability::new());
        let root_cpool_inner = CpoolInner {
            unsafe_data: [NONE_INNER; 256],
        };
        let root_cpool = Cpool {
            linked_task: None,
            data: unsafe {
                Boxed::new(PAddrGlobal::new(
                    &root_cpool_inner as *const CpoolInner as u64,
                ))
            },
        };
        let untyped_ref = RefCell::new(untyped_memory);
        let untyped_unsafe_ref = unsafe { UnsafeRef::from_raw(&untyped_ref) };
        let mut untyped = untyped_unsafe_ref.as_untyped_memory_mut().unwrap();

        let rcpool_cap = Capability {
            capability_data: CapabilityEnum::Cpool(root_cpool),
            ..Default::default()
        };
        let cpool_ref = RefCell::new(rcpool_cap);
        let cpool_unsafe_ref = unsafe { UnsafeRef::from_raw(&cpool_ref) };
        let mut cpool = cpool_unsafe_ref.as_cpool_mut().unwrap();

        let l4 = StoredCap::pml4_retype_from(&mut untyped, &mut cpool).unwrap();
        let raw_page =
            StoredCap::base_page_retype_from::<[u8; 10]>(&mut untyped, &mut cpool).unwrap();
        let mut l4_0 = l4.0.as_l4_mut().unwrap();
        l4_0.l4_map(
            0u64.into(),
            &raw_page.0,
            &mut untyped,
            &mut cpool,
            MapPermissions::WRITE,
        )
        .unwrap();

        // We need 5 caps until now: l4, raw, l3, l2, l1
        assert!(matches!(
            root_cpool_inner.unsafe_data[4].borrow().capability_data,
            CapabilityEnum::L1(..)
        ));
        assert!(matches!(
            root_cpool_inner.unsafe_data[5].borrow().capability_data,
            CapabilityEnum::EmptyCap
        ));

        let raw_page2 =
            StoredCap::large_page_retype_from::<[u8; 10]>(&mut untyped, &mut cpool).unwrap();

        let _fail_map = l4_0
            .l4_map(
                0x1000u64.into(),
                &raw_page2.0,
                &mut untyped,
                &mut cpool,
                MapPermissions::WRITE,
            )
            .unwrap_err();
        assert_matches!(CapabilityErrors::MemoryAlignmentFailure, _fail_map);

        let _fail_map = l4_0
            .l4_map(
                0x0u64.into(),
                &raw_page2.0,
                &mut untyped,
                &mut cpool,
                MapPermissions::WRITE,
            )
            .unwrap_err();
        assert_matches!(CapabilityErrors::MemoryAlreadyMapped, _fail_map);

        l4_0.l4_map(
            0x20_0000u64.into(),
            &raw_page2.0,
            &mut untyped,
            &mut cpool,
            MapPermissions::WRITE,
        )
        .unwrap();

        // We need 6 caps until now: l4, raw, l3, l2, l1, raw2
        assert_matches!(
            root_cpool_inner.unsafe_data[5].borrow().capability_data,
            CapabilityEnum::LargePage(..)
        );
        assert!(matches!(
            root_cpool_inner.unsafe_data[6].borrow().capability_data,
            CapabilityEnum::EmptyCap
        ));
    }
}
