use core::mem::MaybeUninit;

use crate::{prelude::CAddr, SetDefault};

/// Represents a task buffer used for system calls.
#[derive(Debug)]
#[repr(C)]
pub struct TaskBuffer {
    /// Address of the current buffer.
    pub self_address: u64,

    /// Payload information when system call requires it.
    pub payload_length: usize,
    pub payload_data: [u8; 2048],

    /// Capability information when system call requires it.
    pub caps: [Option<CAddr>; 32],

    pub raw_message: u64,
}

impl TaskBuffer {
    /// Write the data into the payload data. Will fail if data size is larger
    /// than the available buffer size.
    pub fn write_to_task_buffer<T>(&mut self, data: &T) -> Result<(), ()> {
        let size = core::mem::size_of_val(data);
        if size > self.payload_data.len() {
            return Err(());
        }

        let val = unsafe {
            core::slice::from_raw_parts((data as *const T) as *const u8, core::mem::size_of::<T>())
        };
        debug_assert_eq!(size, val.len());
        self.payload_data[..size].copy_from_slice(val);
        self.payload_length = size;

        Ok(())
    }

    /// Read from task buffer as type T. Can fail if payload length mismatches.
    pub unsafe fn read_from_task_buffer<T>(&self) -> Result<T, ()> {
        let data = &self.payload_data[..self.payload_length];
        if data.len() != core::mem::size_of::<T>() {
            return Err(());
        }

        let mut result: MaybeUninit<T> = MaybeUninit::uninit();

        let d = &data[0] as *const u8 as *const T;
        core::ptr::copy(d, result.as_mut_ptr(), 1);
        Ok(result.assume_init())
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_buffer() {
        let mut buffer = TaskBuffer {
            self_address: 0,
            raw_message: 0,
            caps: [None; 32],
            payload_length: 0,
            payload_data: [0; 2048],
        };

        let test_data: u64 = 112344;
        buffer.write_to_task_buffer(&test_data).unwrap();
        assert_eq!(8, buffer.payload_length);
        let result: u64 = unsafe { buffer.read_from_task_buffer().unwrap() };
        assert_eq!(test_data, result);
    }
}
