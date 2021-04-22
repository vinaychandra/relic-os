use crate::{arch::paging::table::*, update::capability::CapRcBoxed, util::boxed::Boxed};

pub mod page_cap;

#[derive(Debug)]
pub enum ArchCap {
    RawPageCap {
        page_data: Boxed<[u8; 4096]>,
        mapped_page: Option<CapRcBoxed>,

        next: Option<CapRcBoxed>,
        prev: Option<CapRcBoxed>,
    },
    PML4 {
        page_data: Boxed<[PML4Entry; 512]>,
        mapped_page: Option<CapRcBoxed>,

        next: Option<CapRcBoxed>,
        prev: Option<CapRcBoxed>,
    },
    PDPT {
        page_data: Boxed<[PDPTEntry; 512]>,
        mapped_page: Option<CapRcBoxed>,

        next: Option<CapRcBoxed>,
        prev: Option<CapRcBoxed>,
    },
    PD {
        page_data: Boxed<[PDEntry; 512]>,
        mapped_page: Option<CapRcBoxed>,

        next: Option<CapRcBoxed>,
        prev: Option<CapRcBoxed>,
    },
    PT {
        page_data: Boxed<[PTEntry; 512]>,
        mapped_page: Option<CapRcBoxed>,

        next: Option<CapRcBoxed>,
        prev: Option<CapRcBoxed>,
    },
}

impl ArchCap {
    pub fn get_next(&self) -> &Option<CapRcBoxed> {
        match self {
            ArchCap::RawPageCap { next, .. } => next,
            ArchCap::PML4 { next, .. } => next,
            ArchCap::PDPT { next, .. } => next,
            ArchCap::PD { next, .. } => next,
            ArchCap::PT { next, .. } => next,
        }
    }

    pub fn get_next_mut(&mut self) -> &mut Option<CapRcBoxed> {
        match self {
            ArchCap::RawPageCap { next, .. } => next,
            ArchCap::PML4 { next, .. } => next,
            ArchCap::PDPT { next, .. } => next,
            ArchCap::PD { next, .. } => next,
            ArchCap::PT { next, .. } => next,
        }
    }

    pub fn get_prev(&self) -> &Option<CapRcBoxed> {
        match self {
            ArchCap::RawPageCap { prev, .. } => prev,
            ArchCap::PML4 { prev, .. } => prev,
            ArchCap::PDPT { prev, .. } => prev,
            ArchCap::PD { prev, .. } => prev,
            ArchCap::PT { prev, .. } => prev,
        }
    }

    pub fn get_prev_mut(&mut self) -> &mut Option<CapRcBoxed> {
        match self {
            ArchCap::RawPageCap { prev, .. } => prev,
            ArchCap::PML4 { prev, .. } => prev,
            ArchCap::PDPT { prev, .. } => prev,
            ArchCap::PD { prev, .. } => prev,
            ArchCap::PT { prev, .. } => prev,
        }
    }
}
