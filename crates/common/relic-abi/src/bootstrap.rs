use crate::prelude::CAddr;

/// Info from the kernel to the sigma space. This provides the initial
/// data needed for the sigma process.
#[repr(C)]
#[derive(Debug, Default)]
pub struct BootstrapInfo {
    /// List of free memory regions.
    /// [StartCaddr, EndCAddr]
    pub free_mem_regions: (CAddr, CAddr),

    /// Top level page table for this task.
    pub top_level_pml4: CAddr,

    pub frame_buffer_paddr: u64,
    pub frame_buffer_vaddr: u64,
    pub frame_buffer_size: usize,
    pub frame_buffer_width: usize,
    pub frame_buffer_height: usize,
    pub frame_buffer_scanline: usize,
    pub frame_buffer_mode: ColorMode,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub enum ColorMode {
    ARGB = 0,
    RGBA = 1,
    ABGR = 2,
    BGRA = 3,
}

impl Default for ColorMode {
    fn default() -> Self {
        Self::ARGB
    }
}
