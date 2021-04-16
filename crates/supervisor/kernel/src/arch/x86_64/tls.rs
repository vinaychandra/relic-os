use std::ptr;

use crate::{
    addr::{PAddr, VAddr},
    arch::globals::MEM_MAP_OFFSET_LOCATION,
    util::memory_region::MemoryRegion,
};
use heapless::Vec;

extern "C" {
    static mut __tdata_start: usize;
    static mut __tdata_end: usize;
    static mut __tbss_start: usize;
    static mut __tbss_end: usize;
}

/// Initialize CPU local store for kernel.
/// This can be called per-CPU for TLS data for the core.
pub fn initialize_tls(free_regions: &mut Vec<MemoryRegion, heapless::consts::U32>) {
    let total_size;

    let allocate_data = |size: usize, align: usize| {
        for region in free_regions {
            let allocated = region.try_allocate(size, align);

            if let Some(addr) = allocated {
                return addr;
            }
        }

        panic!("Not enough memory");
    };
    let paddr_to_vaddr = |a: PAddr| {
        let inner: u64 = a.into();
        let target = inner + MEM_MAP_OFFSET_LOCATION;
        VAddr::new(target)
    };

    let tls_ptr: u64 = unsafe {
        let tdata_size =
            &__tdata_end as *const usize as usize - &__tdata_start as *const usize as usize;
        total_size = &__tbss_end as *const usize as usize - &__tdata_start as *const usize as usize;

        let start_paddr = allocate_data(total_size + 8, 2);
        let start_vaddr = paddr_to_vaddr(start_paddr);

        load_tls_data(
            start_vaddr.into(),
            &__tdata_start as *const usize as *const u8,
            tdata_size,
            total_size + 8, // Add 8 bytes to store TCB pointer.
        );

        start_vaddr.into()
    };
    info!(target: "initialize_tls", "TLS data loaded. Setting fs");
    let fs_ptr = ((tls_ptr as *const u8 as u64) + (total_size as u64)) as *mut u64;
    x86_64::registers::model_specific::FsBase::write(x86_64::VirtAddr::from_ptr(fs_ptr));
    x86_64::registers::model_specific::KernelGsBase::write(x86_64::VirtAddr::from_ptr(fs_ptr));
    unsafe {
        // SystemV Abi needs [fs:0] to be the value of fs
        *fs_ptr = fs_ptr as u64;
    }

    info!(target: "initialize_tls", "TLS Pointer is set to {:x?}. Size is {:?} bytes", fs_ptr, total_size);
}

/// Load TLS data into memory and return its physical address.
/// All sizes are in bytes.
/// # Arguments
/// - `start_addr`: Starting virtual address for TLS segment.
/// - `tdata_size`: The number of data bytes in the template. Corresponds to
///         the length of the `.tdata` section.
/// - `total_size`: The total number of bytes that the TLS segment should have in memory.
///         Generally corresponds to the combined length of the `.tdata` and `.tbss` sections.
/// # Returns
/// Virtual address of the target pointer. The data will be of size `total_size`. The `tdata`
/// will be in the first required bytes of the returned array.
pub unsafe fn load_tls_data(
    vaddr_location_to_store: u64,
    start_addr: *const u8,
    tdata_size: usize,
    total_size: usize,
) {
    // We add 8 bytes to have storage to store fs pointer.
    ptr::copy(start_addr, vaddr_location_to_store as *mut u8, tdata_size);
    ptr::write_bytes(
        ((vaddr_location_to_store as usize) + tdata_size) as *mut u8,
        0,
        total_size - tdata_size,
    );
}
