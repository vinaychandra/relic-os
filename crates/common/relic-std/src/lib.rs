#![cfg_attr(not(test), no_std)]
#![cfg_attr(test, allow(unused_imports))]
#![feature(asm)]

#[cfg(not(test))]
extern crate core as std;

pub mod raw_syscall;

use std::panic::PanicInfo;

use relic_abi::syscall::SystemCall;

/// This function is called on panic.
#[cfg_attr(target_os = "none", panic_handler)]
fn _panic_handler(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
fn _start() -> ! {
    unsafe {
        let _tls: *const ();
        let _tls2: *const ();
        asm!(
            "mov {0}, fs:0",
            out(reg) _tls
        );
        raw_syscall::make_syscall(&SystemCall::Yield).unwrap();
        let _tls2: *const ();
        asm!(
            "mov {0}, fs:0",
            out(reg) _tls2
        );
        raw_syscall::make_syscall(&SystemCall::Yield).unwrap();
    }
    loop {}
}
