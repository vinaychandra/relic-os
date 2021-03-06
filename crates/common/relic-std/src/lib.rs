#![cfg_attr(not(test), no_std)]
#![cfg_attr(test, allow(unused_imports))]
#![feature(alloc_error_handler)]
#![feature(alloc_prelude)]
#![feature(asm)]
#![feature(crate_visibility_modifier)]
#![feature(prelude_import)]
#![feature(thread_local)]

extern crate alloc;

pub mod prelude {
    pub use alloc::prelude::v1::*;
    pub use alloc::vec;
    pub use core::prelude::v1::*;
}

pub use alloc::*;
pub use core::*;

#[allow(unused_imports)]
#[prelude_import]
use crate::prelude::*;

pub mod debug;
pub mod heap;
pub mod raw_syscall;
pub mod syscall_wrapper;
pub mod tls;

use core::panic::PanicInfo;

use relic_abi::bootstrap::BootstrapInfo;
use relic_abi::syscall::TaskBuffer;

use crate::{heap::init_heap, tls::load_tls};

/// This function is called on panic.
#[cfg_attr(target_os = "none", panic_handler)]
fn _panic_handler(_info: &PanicInfo) -> ! {
    loop {}
}

#[cfg_attr(target_os = "none", no_mangle)]
pub fn _start() -> ! {
    let bootstrap_info: BootstrapInfo;
    let tcb_ptr: u64;
    unsafe {
        let tcb: *mut TaskBuffer;
        asm!(
            "mov {0}, fs:0",
            out(reg) tcb
        );
        bootstrap_info = (&*tcb).read_from_task_buffer().unwrap();
        tcb_ptr = tcb as _;
    }

    init_heap(&bootstrap_info);
    load_tls(&bootstrap_info, tcb_ptr);

    unsafe { asm!("call user_main", in("rdi") &bootstrap_info) };
    loop {}
}
