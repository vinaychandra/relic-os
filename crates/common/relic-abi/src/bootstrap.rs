use crate::prelude::CAddr;

#[repr(C)]
pub struct BootstrapInfo {
    /// Address of the capability pool for the task.
    pub cpool_capability: CAddr,

    /// List of free memory regions.
    /// [StartCaddr, EndCAddr]
    pub free_mem_regions: (CAddr, CAddr),

    /// Top level page table for the task.
    pub top_level_pml4: CAddr,
}
