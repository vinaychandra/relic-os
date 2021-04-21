#![cfg_attr(not(test), no_std)]
#![cfg_attr(test, allow(unused_imports))]
#![feature(asm)]

#[cfg(not(test))]
extern crate core as std;

pub mod debug;
pub mod raw_syscall;
pub mod syscall_wrapper;

use std::panic::PanicInfo;

use relic_abi::bootstrap::BootstrapInfo;
use relic_abi::syscall::TaskBuffer;

/// This function is called on panic.
#[cfg_attr(target_os = "none", panic_handler)]
fn _panic_handler(_info: &PanicInfo) -> ! {
    loop {}
}

#[cfg_attr(target_os = "none", no_mangle)]
fn _start() -> ! {
    let bootstrap_info: BootstrapInfo;
    unsafe {
        let tls: *mut TaskBuffer;
        asm!(
            "mov {0}, fs:0",
            out(reg) tls
        );
        bootstrap_info = (&*tls).read_from_task_buffer().unwrap();
    }
    let raw_page = crate::syscall_wrapper::retype_raw_page(1.into()).unwrap();

    let location = 0x1_0000_0000;
    let c = crate::syscall_wrapper::map_raw_page(
        1.into(),
        bootstrap_info.top_level_pml4,
        location,
        raw_page,
    );
    let _c = c;
    loop {}
}
