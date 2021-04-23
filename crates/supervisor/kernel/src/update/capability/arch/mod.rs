use intrusive_collections::UnsafeRef;

use crate::{arch::paging::table::*, util::boxed::Boxed};

use super::*;
pub mod pd;
// pub mod pdpt;
// pub mod pml4;

#[derive(Debug)]
pub enum ArchCap {
    PML4 {
        page_data: Boxed<[PML4Entry; 512]>,
        children: LinkedList<PagingTreeAdapter>,
    },
    PDPT {
        page_data: Boxed<[PDPTEntry; 512]>,
        parent_pml4: Option<UnsafeRef<Capability>>,
    },
    PD {
        page_data: Boxed<[PDEntry; 512]>,
        parent_pml4: Option<UnsafeRef<Capability>>,
    },
    PT {
        page_data: Boxed<[PTEntry; 512]>,
        parent_pml4: Option<UnsafeRef<Capability>>,
    },
    RawPageCap {
        page_data: Boxed<[u8; 4096]>,
        parent_pml4: Option<UnsafeRef<Capability>>,
    },
}
