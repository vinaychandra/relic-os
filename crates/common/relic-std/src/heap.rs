use core::alloc::Layout;

use buddy_system_allocator::LockedHeapWithRescue;

#[cfg_attr(target_os = "none", global_allocator)]
pub static HEAP: LockedHeapWithRescue<20> = LockedHeapWithRescue::new(expand_heap);

fn expand_heap(_heap: &mut buddy_system_allocator::Heap<20>, _layout: &Layout) {}

#[cfg_attr(target_os = "none", alloc_error_handler)]
fn _alloc_error_handler(layout: alloc::alloc::Layout) -> ! {
    panic!("allocation error: {:?}", layout);
}
