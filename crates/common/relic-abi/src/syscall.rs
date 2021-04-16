use crate::{prelude::CAddr, SetDefault};

/// Represents a task buffer used for system calls.
#[derive(Debug)]
#[repr(C)]
pub struct TaskBuffer {
    /// Address of the current buffer.
    pub self_address: u64,

    /// Payload information when system call requires it.
    pub payload_length: usize,
    pub payload_data: [u8; 1024],

    /// Capability information when system call requires it.
    pub caps: [Option<CAddr>; 32],

    pub raw_message: u64,
}

impl SetDefault for TaskBuffer {
    fn set_default(&mut self) {}
}

#[derive(Debug, Clone)]
#[repr(C)]
#[non_exhaustive]
pub enum SystemCall {
    None,  // 0
    Yield, // 1
}

impl Default for SystemCall {
    fn default() -> Self {
        Self::None
    }
}

impl SystemCall {
    pub fn as_regs(&self) -> Result<(u64, u64, u64, u64, u64), ()> {
        match self {
            SystemCall::Yield => Ok((1, 0, 0, 0, 0)),
            _ => Err(()),
        }
    }

    #[allow(unused_variables)]
    pub fn from_regs(a: u64, b: u64, c: u64, d: u64, e: u64) -> Result<SystemCall, ()> {
        match a {
            1 => Ok(SystemCall::Yield),
            _ => Err(()),
        }
    }
}
