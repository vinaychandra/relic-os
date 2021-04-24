use relic_abi::cap::CapabilityErrors;
use std::cell::RefCell;

use crate::{addr::PAddrGlobal, arch::paging::table::*};
use intrusive_collections::*;

mod arch;
mod cpool;
mod untyped;

pub use arch::*;
pub use cpool::*;
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
    RawPage(RawPage),
    // Arch(arch::ArchCap),
}

#[derive(Debug)]
pub struct Capability {
    capability_data: RefCell<CapabilityEnum>,
    mem_tree_link: LinkedListLink,
    paging_tree_link: LinkedListLink,
}

intrusive_adapter!(pub MemTreeAdapter = UnsafeRef<Capability>: Capability { mem_tree_link: LinkedListLink });
intrusive_adapter!(pub PagingTreeAdapter = UnsafeRef<Capability>: Capability { paging_tree_link: LinkedListLink });

macro_rules! cap_create {
    ($data: tt) => {
        paste! {
            impl Capability {
                pub fn [< $data:snake _create >]<F, R>(&self, f: F) -> Result<R, CapabilityErrors>
                where
                    F: FnOnce(&$data) -> Result<R, CapabilityErrors>,
                {
                    let cap = self.capability_data.borrow();
                    if let CapabilityEnum::$data(data) = &*cap {
                        f(data)
                    } else {
                        Err(CapabilityErrors::CapabilityMismatch)
                    }
                }

                pub fn [< $data:snake _create_mut >]<F, R>(&self, f: F) -> Result<R, CapabilityErrors>
                where
                    F: FnOnce(&mut $data) -> Result<R, CapabilityErrors>,
                {
                    let mut cap = self.capability_data.borrow_mut();
                    if let CapabilityEnum::$data(data) = &mut *cap {
                        f(data)
                    } else {
                        Err(CapabilityErrors::CapabilityMismatch)
                    }
                }

                pub fn [< $data:snake _get_mut >]<F, R>(&mut self, f: F) -> Result<R, CapabilityErrors>
                where
                    F: FnOnce(&mut $data) -> Result<R, CapabilityErrors>,
                {
                    let cap = self.capability_data.get_mut();
                    if let CapabilityEnum::$data(data) = cap {
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
cap_create!(L4);
cap_create!(L3);
cap_create!(L2);
cap_create!(L1);
cap_create!(RawPage);
