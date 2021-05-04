use relic_abi::bootstrap::BootstrapInfo;

use crate::syscall_wrapper;

crate fn load_tls(bootstrap_info: &BootstrapInfo, tcb_ptr: u64) {
    let tls_info = &bootstrap_info.tls_info;

    if !tls_info.tls_loaded {
        return;
    }

    let num_pages = relic_utils::align::align_up(tls_info.total_size as usize, 4096 as _) / 4096;

    let start_addr: u8 = bootstrap_info.free_mem_regions.0 .0[0];
    let end_addr: u8 = bootstrap_info.free_mem_regions.1 .0[0];

    let mut target_addr = None;
    for addr in start_addr..=end_addr {
        let free_sapce = syscall_wrapper::get_free_space(addr.into()).unwrap();
        // 3 is arbitariry chosen to make sure we have enough extra space for any other page tables.
        if free_sapce.1 > (num_pages + 3) * 4096 {
            target_addr = Some(addr);
            break;
        }
    }

    if target_addr.is_none() {
        panic!("No memory space enough to hold TLS");
    }

    // Allocate empty data right before
    for page_index in 0..num_pages {
        let raw_page = syscall_wrapper::retype_raw_page(target_addr.unwrap().into(), 0).unwrap();
        let target_vaddr = tcb_ptr - (page_index as u64 + 1) * 4096;

        syscall_wrapper::map_raw_page(
            target_addr.unwrap().into(),
            bootstrap_info.top_level_pml4,
            target_vaddr,
            raw_page,
        )
        .unwrap();
    }

    // Now all the addresses are mapped into the current address space. Now, we copy the tdata image.
    let tdata_size_with_align =
        relic_utils::align::align_up(tls_info.total_size, tls_info.tls_align);
    // This data is present before the tcb ptr.
    let address_to_start_writing_at = tcb_ptr - tdata_size_with_align;
    let address_to_start_reading_at = tls_info.tdata_start;
    unsafe {
        core::ptr::copy(
            address_to_start_reading_at as *mut u8,
            address_to_start_writing_at as *mut u8,
            tls_info.tdata_length as _,
        )
    };
}
