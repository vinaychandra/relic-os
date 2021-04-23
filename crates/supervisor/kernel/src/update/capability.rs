use std::cell::RefCell;

use crate::{addr::PAddrGlobal, util::boxed::Boxed};
use intrusive_collections::*;

// pub mod arch;
mod cpool;
mod untyped;

pub use cpool::*;
pub use untyped::*;

type Size = usize;

#[derive(Debug)]
pub enum CapabilityEnum {
    UntypedMemory {
        start_paddr: PAddrGlobal,
        length: Size,
        watermark: PAddrGlobal,

        children: LinkedList<MemTreeAdapter>,
    },
    CPool {
        data: Boxed<CPoolInner>,
    },
    EmptyCap,
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
