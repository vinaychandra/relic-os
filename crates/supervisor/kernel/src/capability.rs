use relic_abi::cap::CapabilityErrors;
use std::cell::{Ref, RefCell, RefMut};

use crate::{addr::PAddrGlobal, arch::capability::paging::*, util::unsafe_ref::UnsafeRef};

mod cpool;
pub mod task;
mod untyped;

pub use cpool::*;
pub use task::*;
pub use untyped::*;

type Size = usize;

#[derive(Debug)]
pub enum CapabilityEnum {
    UntypedMemory(UntypedMemory),
    Cpool(Cpool),
    EmptyCap,

    L4(L4),
    L3(L3),
    L2(L2),
    L1(L1),

    BasePage(BasePage),
    LargePage(LargePage),
    HugePage(HugePage),

    Task(Task),
}

pub type BasePage = RawPageActual<0x1000>;
pub type LargePage = RawPageActual<0x20_0000>;
pub type HugePage = RawPageActual<0x4000_0000>;

#[derive(Debug)]
pub struct Capability {
    pub capability_data: CapabilityEnum,

    pub next_mem_item: Option<StoredCap>,
    pub prev_mem_item: Option<StoredCap>,
}

// fixed 64 byte sized assertion for capability.
assert_eq_size!([u8; 64], RefCell<Capability>);

pub type StoredCap = UnsafeRef<RefCell<Capability>>;

impl Default for Capability {
    fn default() -> Self {
        Self::new()
    }
}

impl Capability {
    pub fn get_next_task_item_mut(&mut self) -> &mut Option<StoredCap> {
        match &mut self.capability_data {
            CapabilityEnum::Task(l) => &mut l.next_task_item,
            _ => panic!("Unsupported"),
        }
    }

    pub fn get_prev_task_item_mut(&mut self) -> &mut Option<StoredCap> {
        match &mut self.capability_data {
            CapabilityEnum::Task(l) => &mut l.prev_task_item,
            _ => panic!("Unsupported"),
        }
    }

    pub fn get_next_paging_item_mut(&mut self) -> &mut Option<StoredCap> {
        match &mut self.capability_data {
            CapabilityEnum::L3(l) => &mut l.next_paging_item,
            CapabilityEnum::L2(l) => &mut l.next_paging_item,
            CapabilityEnum::L1(l) => &mut l.next_paging_item,
            CapabilityEnum::BasePage(l) => &mut l.next_paging_item,
            CapabilityEnum::LargePage(l) => &mut l.next_paging_item,
            CapabilityEnum::HugePage(l) => &mut l.next_paging_item,
            _ => panic!("Unsupported"),
        }
    }

    pub fn get_prev_paging_item_mut(&mut self) -> &mut Option<StoredCap> {
        match &mut self.capability_data {
            CapabilityEnum::L3(l) => &mut l.prev_paging_item,
            CapabilityEnum::L2(l) => &mut l.prev_paging_item,
            CapabilityEnum::L1(l) => &mut l.prev_paging_item,
            CapabilityEnum::BasePage(l) => &mut l.prev_paging_item,
            CapabilityEnum::LargePage(l) => &mut l.prev_paging_item,
            CapabilityEnum::HugePage(l) => &mut l.prev_paging_item,
            _ => panic!("Unsupported"),
        }
    }

    pub const fn new() -> Self {
        Self {
            capability_data: CapabilityEnum::EmptyCap,
            next_mem_item: None,
            prev_mem_item: None,
        }
    }
}

#[derive(Getters)]
pub struct CapAccessor<'a, T> {
    _borrow: Ref<'a, Capability>,
    data: *const T,
    #[getset(get = "pub")]
    cap: StoredCap,
}

#[derive(Getters)]
pub struct CapAccessorMut<'a, T> {
    _borrow: RefMut<'a, Capability>,
    data: *mut T,
    #[getset(get = "pub")]
    cap: StoredCap,
}

impl<T> core::ops::Deref for CapAccessor<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // This is safe because this pointer is valid when the borrow is alive.
        unsafe { &*self.data }
    }
}

impl<T> core::ops::Deref for CapAccessorMut<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // This is safe because this pointer is valid when the borrow is alive.
        unsafe { &*self.data }
    }
}

impl<T> core::ops::DerefMut for CapAccessorMut<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // This is safe because this pointer is valid when the borrow is alive.
        unsafe { &mut *self.data }
    }
}

macro_rules! cap_create {
    ($cap_name: ty) => {
        paste! {
            impl StoredCap {
                pub fn [< as_ $cap_name:snake >](&self) -> Result<CapAccessor<'_, $cap_name>, CapabilityErrors> {
                    let borrow = self.borrow();
                    let data = if let CapabilityEnum::$cap_name(u) = &borrow.capability_data {
                        u as *const $cap_name
                    } else {
                        return Err(CapabilityErrors::CapabilityMismatch);
                    };
                    Ok(CapAccessor {
                        _borrow: borrow,
                        data,
                        cap: self.clone(),
                    })
                }

                pub fn [< as_ $cap_name:snake _mut >](
                    &self,
                ) -> Result<CapAccessorMut<'_, $cap_name>, CapabilityErrors> {
                    let mut borrow = self.borrow_mut();
                    let data = if let CapabilityEnum::$cap_name(u) = &mut borrow.capability_data {
                        u as *mut $cap_name
                    } else {
                        return Err(CapabilityErrors::CapabilityMismatch);
                    };
                    Ok(CapAccessorMut {
                        _borrow: borrow,
                        data,
                        cap: self.clone(),
                    })
                }
            }
        }
    };
}

cap_create!(UntypedMemory);
cap_create!(Cpool);
cap_create!(L4);
cap_create!(L3);
cap_create!(L2);
cap_create!(L1);
cap_create!(BasePage);
cap_create!(LargePage);
cap_create!(HugePage);
cap_create!(Task);

bitflags! {
    /// Permissions for the current page.
    pub struct MapPermissions : u8 {
        const WRITE     = 0b0000_0010;
        const EXECUTE   = 0b0000_0100;
    }
}
