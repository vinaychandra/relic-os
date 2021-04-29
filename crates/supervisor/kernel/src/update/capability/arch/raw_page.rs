use std::{any::TypeId, marker::PhantomData};

use relic_abi::cap::CapabilityErrors;

use super::*;
use crate::util::boxed::Boxed;

#[derive(Debug)]
pub struct RawPageActual<T, const SIZE: usize> {
    page_data: Boxed<Inner<SIZE>>,
    pub type_id: TypeId,
    phantom: PhantomData<T>,

    pub next_paging_item: Option<StoredCap>,
    pub prev_paging_item: Option<StoredCap>,
}

#[derive(Debug)]
struct Inner<const SIZE: usize>([u8; SIZE]);

macro_rules! raw_page_impl {
    ($name: ty, $size: tt) => {
        paste! {
            impl StoredCap {
                pub fn [<$name:snake _retype_from>]<T: 'static>(
                    untyped: &mut UntypedMemory,
                    cpool_to_store_in: &mut Cpool,
                ) -> Result<(StoredCap, usize), CapabilityErrors> {
                    assert!(core::mem::size_of::<T>() <= $size);
                    assert!(core::mem::align_of::<T>() <= $size);
                    let mut result_index = 0;

                    let cap = untyped.derive(Some($size), |memory: *mut Inner<$size>| {
                        unsafe {
                            core::ptr::write_bytes(memory as *mut u8, 0, $size);
                        }
                        let boxed = unsafe { Boxed::new((memory as u64).into()) };

                        let stored_index = cpool_to_store_in.get_free_index()?;
                        let cap = cpool_to_store_in.write_to_if_empty(
                            stored_index,
                            Capability {
                                capability_data: CapabilityEnum::$name($name {
                                    next_paging_item: None,
                                    prev_paging_item: None,
                                    page_data: boxed,
                                    phantom: PhantomData,
                                    type_id: TypeId::of::<T>(),
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
        }
    };
}

raw_page_impl!(BasePage, 0x1000);
raw_page_impl!(LargePage, 0x20_0000);
raw_page_impl!(HugePage, 0x4000_0000);

impl<T: 'static, const SIZE: usize> RawPageActual<T, SIZE> {
    pub fn start_paddr(&self) -> PAddrGlobal {
        self.page_data.paddr_global()
    }

    pub fn length(&self) -> usize {
        SIZE
    }

    pub fn page_data(&self) -> &T {
        assert!(TypeId::of::<T>() == self.type_id);
        unsafe { &*(&self.page_data.0[0] as *const u8 as *const T) }
    }

    pub fn page_data_mut(&mut self) -> &mut T {
        assert!(TypeId::of::<T>() == self.type_id);
        unsafe { &mut *(&self.page_data.0[0] as *const u8 as *mut T) }
    }
}
