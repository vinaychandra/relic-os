use core::alloc::Layout;

use buddy_system_allocator::LockedHeapWithRescue;
use relic_abi::bootstrap::BootstrapInfo;

use crate::syscall_wrapper;

#[cfg_attr(target_os = "none", global_allocator)]
pub static HEAP: LockedHeapWithRescue<22> = LockedHeapWithRescue::new(expand_heap);

fn expand_heap(_heap: &mut buddy_system_allocator::Heap<22>, _layout: &Layout) {
    todo!("Heap expansion needs to be implemetned.")
}

#[cfg_attr(target_os = "none", alloc_error_handler)]
fn _alloc_error_handler(layout: alloc::alloc::Layout) -> ! {
    panic!("allocation error: {:?}", layout);
}

const HEAP_LOCATION: u64 = 0x1_0000_0000;

crate fn init_heap(bootstrap_info: &BootstrapInfo) {
    // Allocate a 2MiB heap
    let start_addr: u8 = bootstrap_info.free_mem_regions.0 .0[0];
    let end_addr: u8 = bootstrap_info.free_mem_regions.1 .0[0];

    let mut target_addr = None;
    for addr in start_addr..=end_addr {
        let free_sapce = syscall_wrapper::get_free_space(addr.into()).unwrap();
        if free_sapce.1 > 0x20_0000 {
            target_addr = Some(addr);
            break;
        }
    }

    if target_addr.is_none() {
        panic!("No memory space enough to hold heap");
    }

    //TODO: This can fail if available space is exactly enough and system needs more space to create
    // more paging structures.
    let raw_page = syscall_wrapper::retype_raw_page(target_addr.unwrap().into(), 1).unwrap();
    syscall_wrapper::map_raw_page(
        target_addr.unwrap().into(),
        bootstrap_info.top_level_pml4,
        HEAP_LOCATION,
        raw_page,
    )
    .unwrap();

    unsafe { HEAP.lock().init(HEAP_LOCATION as _, 0x20_0000) };
}
