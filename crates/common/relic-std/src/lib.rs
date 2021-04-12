#![cfg_attr(not(test), no_std)]
#![cfg_attr(test, allow(unused_imports))]

#[cfg(not(test))]
extern crate core as std;

use std::panic::PanicInfo;

/// This function is called on panic.
#[cfg_attr(target_os = "none", panic_handler)]
fn _panic_handler(_info: &PanicInfo) -> ! {
    loop {}
}
