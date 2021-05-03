#![cfg_attr(not(test), no_std)]
#![cfg_attr(test, allow(unused_imports))]
#![feature(alloc_error_handler)]
#![feature(alloc_prelude)]
#![feature(asm)]
#![feature(crate_visibility_modifier)]
#![feature(prelude_import)]

extern crate alloc;

mod prelude {
    pub use alloc::prelude::v1::*;
    pub use core::prelude::v1::*;
}

#[allow(unused_imports)]
#[prelude_import]
use crate::prelude::*;

pub mod debug;
pub mod heap;
pub mod raw_syscall;
pub mod syscall_wrapper;

use core::panic::PanicInfo;

use relic_abi::bootstrap::BootstrapInfo;
use relic_abi::syscall::TaskBuffer;

use crate::heap::init_heap;

/// This function is called on panic.
#[cfg_attr(target_os = "none", panic_handler)]
fn _panic_handler(_info: &PanicInfo) -> ! {
    loop {}
}

#[cfg_attr(target_os = "none", no_mangle)]
pub fn _start() -> ! {
    let bootstrap_info: BootstrapInfo;
    unsafe {
        let tls: *mut TaskBuffer;
        asm!(
            "mov {0}, fs:0",
            out(reg) tls
        );
        bootstrap_info = (&*tls).read_from_task_buffer().unwrap();
    }

    init_heap(&bootstrap_info);

    let _a = Box::new(10);
    loop {}
}
