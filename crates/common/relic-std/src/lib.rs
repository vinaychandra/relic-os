#![cfg_attr(not(test), no_std)]
#![cfg_attr(test, allow(unused_imports))]
#![feature(asm)]

#[cfg(not(test))]
extern crate core as std;

pub mod debug;
pub mod raw_syscall;
pub mod syscall_wrapper;

use std::panic::PanicInfo;

/// This function is called on panic.
#[cfg_attr(target_os = "none", panic_handler)]
fn _panic_handler(_info: &PanicInfo) -> ! {
    loop {}
}

#[cfg_attr(target_os = "none", no_mangle)]
fn _start() -> ! {
    let a = crate::syscall_wrapper::get_free_space(1.into());
    let b = a.unwrap();
    let _c = b;
    loop {}
}
