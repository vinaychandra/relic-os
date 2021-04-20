use relic_abi::{
    cap::CapabilityErrors,
    prelude::CAddr,
    syscall::{SystemCall, TaskBuffer},
};

use crate::raw_syscall;

pub fn get_free_space(cap: CAddr) -> Result<(usize, usize), CapabilityErrors> {
    let syscall = SystemCall::UntypedTotalFree(cap);
    raw_syscall::make_syscall(&syscall)?;
    let data: (usize, usize) = unsafe { (&*get_task_buffer()).read_from_task_buffer().unwrap() };
    Ok(data)
}

unsafe fn get_task_buffer() -> *mut TaskBuffer {
    let tls: *mut TaskBuffer;
    asm!(
        "mov {0}, fs:0",
        out(reg) tls
    );

    tls
}
