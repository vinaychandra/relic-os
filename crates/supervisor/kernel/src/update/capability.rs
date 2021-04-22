use crate::{
    addr::PAddrGlobal,
    update::capability::arch::ArchCap,
    util::boxed::{Boxed, RcRefCellBoxed},
};

pub mod arch;
mod cpool;
mod untyped;

pub use cpool::*;
pub use untyped::*;

type Size = usize;

#[derive(Debug)]
pub enum Capability {
    UntypedMemory {
        start_paddr: PAddrGlobal,
        length: Size,
        watermark: PAddrGlobal,

        first_child_item: Option<CapRcBoxed>,

        next: Option<CapRcBoxed>,
        prev: Option<CapRcBoxed>,
    },
    CPool {
        data: Boxed<CPoolInner>,
        next: Option<CapRcBoxed>,
        prev: Option<CapRcBoxed>,
    },
    EmptyCap {
        next: Option<CapRcBoxed>,
        prev: Option<CapRcBoxed>,
    },
    Arch(ArchCap),
}
pub type CapRcBoxed = RcRefCellBoxed<Capability>;

impl Default for Capability {
    fn default() -> Self {
        Capability::EmptyCap {
            next: None,
            prev: None,
        }
    }
}

impl Capability {
    pub fn get_next(&self) -> &Option<CapRcBoxed> {
        match self {
            Capability::UntypedMemory { next, .. } => next,
            Capability::CPool { next, .. } => next,
            Capability::EmptyCap { next, .. } => next,
            Capability::Arch(arch) => arch.get_next(),
        }
    }

    pub fn get_next_mut(&mut self) -> &mut Option<CapRcBoxed> {
        match self {
            Capability::UntypedMemory { next, .. } => next,
            Capability::CPool { next, .. } => next,
            Capability::EmptyCap { next, .. } => next,
            Capability::Arch(arch) => arch.get_next_mut(),
        }
    }

    pub fn get_prev(&self) -> &Option<CapRcBoxed> {
        match self {
            Capability::UntypedMemory { prev: prev_ptr, .. } => prev_ptr,
            Capability::CPool { prev, .. } => prev,
            Capability::EmptyCap { prev, .. } => prev,
            Capability::Arch(arch) => arch.get_prev(),
        }
    }

    pub fn get_prev_mut(&mut self) -> &mut Option<CapRcBoxed> {
        match self {
            Capability::UntypedMemory { prev, .. } => prev,
            Capability::CPool { prev, .. } => prev,
            Capability::EmptyCap { prev, .. } => prev,
            Capability::Arch(arch) => arch.get_prev_mut(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::boxed::RcRefCellBoxedInner;

    #[test]
    fn test_cap_size() {
        assert_eq!(64, core::mem::size_of::<RcRefCellBoxedInner<Capability>>());
    }
}
