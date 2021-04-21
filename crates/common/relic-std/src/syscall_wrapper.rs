use relic_abi::{
    cap::CapabilityErrors,
    prelude::CAddr,
    syscall::{SystemCall, TaskBuffer},
};

use crate::raw_syscall;

pub fn get_free_space(cap: CAddr) -> Result<(usize, usize), CapabilityErrors> {
    let syscall = SystemCall::UntypedTotalFree(cap);
    raw_syscall::make_syscall(&syscall).map(|(a, b)| (a as usize, b as usize))
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
