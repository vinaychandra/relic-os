use crate::prelude::CAddr;

/// Info from the kernel to the sigma space. This provides the initial
/// data needed for the sigma process.
#[repr(C)]
pub struct BootstrapInfo {
    /// Address of the capability pool for this task.
    pub cpool_capability: CAddr,

    /// List of free memory regions.
    /// [StartCaddr, EndCAddr]
    pub free_mem_regions: (CAddr, CAddr),

    /// Top level page table for this task.
    pub top_level_pml4: CAddr,
}
