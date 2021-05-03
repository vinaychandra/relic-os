use relic_abi::{
    cap::CapabilityErrors,
    prelude::CAddr,
    syscall::{SystemCall, TaskBuffer},
};

use crate::raw_syscall;

/// Get total size and free size for an untyped capability.
pub fn get_free_space(cap: CAddr) -> Result<(usize, usize), CapabilityErrors> {
    let syscall = SystemCall::UntypedTotalFree(cap);
    raw_syscall::make_syscall(&syscall).map(|(a, b)| (a as usize, b as usize))
}

/// Retype untyped memory into a raw page and returns its CAddr.
/// The value of size is the 'type' of page. This is architecture
/// dependant. Example: 0 => 4KiB, 1 => 2MiB, 2 => 1GiB.
pub fn retype_raw_page(cap: CAddr, size_type: u64) -> Result<CAddr, CapabilityErrors> {
    let syscall = SystemCall::RawPageRetype {
        untyped_memory: cap,
        size: size_type,
    };
    raw_syscall::make_syscall(&syscall).map(|(a, _)| (a as u8).into())
}

/// Map a given page into the provided address.
/// Parameters:
/// * `untyped_memory` - To map raw pages, we might need more pages for inner tables.
/// * `top_level_table` - The top level table into which the mapping should be done.
/// * `vaddr` - The address where the mapping should be done to.
/// * `raw_page` - The raw page capability for the request.
pub fn map_raw_page(
    untyped_memory: CAddr,
    top_level_table: CAddr,
    vaddr: u64,
    raw_page: CAddr,
) -> Result<(), CapabilityErrors> {
    let syscall = SystemCall::RawPageMap {
        raw_page,
        vaddr,
        untyped_memory,
        top_level_table,
    };
    raw_syscall::make_syscall(&syscall).map(|(_, _)| ())
}

#[allow(dead_code)]
unsafe fn get_task_buffer() -> *mut TaskBuffer {
    let tls: *mut TaskBuffer;
    asm!(
        "mov {0}, fs:0",
        out(reg) tls
    );

    tls
}
