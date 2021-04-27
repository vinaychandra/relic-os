use relic_abi::cap::CapabilityErrors;
use std::cell::RefCell;

use crate::{
    addr::PAddrGlobal, arch::paging::table::*, update::unsafe_ref::UnsafeRef, util::boxed::Boxed,
};

// mod arch;
mod cpool;
mod untyped;

// pub use arch::*;
pub use cpool::*;
pub use untyped::*;

type Size = usize;

#[derive(Debug)]
pub enum CapabilityEnum {
    UntypedMemory(UntypedMemory),
    Cpool(Cpool),
    EmptyCap,
}

#[derive(Debug)]
pub struct Capability {
    capability_data: CapabilityEnum,

    pub next_mem_item: Option<StoredCap>,
    pub prev_mem_item: Option<StoredCap>,
}

pub type StoredCap = UnsafeRef<RefCell<Capability>>;

impl Default for Capability {
    fn default() -> Self {
        Self::new()
    }
}

impl Capability {
    pub const fn new() -> Self {
        Self {
            capability_data: CapabilityEnum::EmptyCap,
            next_mem_item: None,
            prev_mem_item: None,
        }
    }
}

macro_rules! cap_create {
    ($data: tt) => {
        paste! {
            impl StoredCap {
                pub fn [< $data:snake _create >]<F, R>(&self, f: F) -> Result<R, CapabilityErrors>
                where
                    F: FnOnce(&$data) -> Result<R, CapabilityErrors>,
                {
                    let cap = self.borrow();
                    if let CapabilityEnum::$data(data) = &cap.capability_data {
                        f(data)
                    } else {
                        Err(CapabilityErrors::CapabilityMismatch)
                    }
                }

                pub fn [< $data:snake _create_mut >]<F, R>(&self, f: F) -> Result<R, CapabilityErrors>
                where
                    F: FnOnce(&mut $data) -> Result<R, CapabilityErrors>,
                {
                    let mut cap = self.borrow_mut();
                    if let CapabilityEnum::$data(data) = &mut cap.capability_data {
                        f(data)
                    } else {
                        Err(CapabilityErrors::CapabilityMismatch)
                    }
                }
            }
        }
    };
}

cap_create!(UntypedMemory);
cap_create!(Cpool);
// cap_create!(L4);
// cap_create!(L3);
// cap_create!(L2);
// cap_create!(L1);
// cap_create!(RawPage);
