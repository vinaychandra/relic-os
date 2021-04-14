use crate::arch::capability::paging::PML4Cap;

pub mod paging;

/// The top-level page table capability. In `x86_64`, this is PML4.
pub type TopPageTableCap = PML4Cap;
