#![cfg_attr(not(test), no_std)]
#![cfg_attr(test, allow(unused_imports))]
#![feature(asm)]

#[cfg(not(test))]
extern crate core as std;

pub mod raw_syscall;

use std::panic::PanicInfo;

use relic_abi::{bootstrap::BootstrapInfo, syscall::TaskBuffer};

/// This function is called on panic.
#[cfg_attr(target_os = "none", panic_handler)]
fn _panic_handler(_info: &PanicInfo) -> ! {
    loop {}
}

#[cfg_attr(target_os = "none", no_mangle)]
fn _start() -> ! {
    unsafe {
        let tls: *const TaskBuffer;
        asm!(
            "mov {0}, fs:0",
            out(reg) tls
        );

        let bootstrap_info: BootstrapInfo = (*tls).read_from_task_buffer().unwrap();
        let _b = bootstrap_info;
    }
    loop {}
}
