use core::mem::MaybeUninit;

use crate::{prelude::CAddr, SetDefault};

/// Represents a task buffer used for system calls.
#[derive(Debug)]
#[repr(C)]
pub struct TaskBuffer {
    /// Address of the current buffer.
    pub self_address: u64,

    /// Payload length for a syscall.
    pub payload_length: usize,
    /// Payload data in the task buffer. Only data upto
    /// [`Self::payload_length`] is valid when used.
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

    /// Read from task buffer as type T. Will fail if payload length mismatches.
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
    fn set_default(&mut self) {
        self.payload_length = 0;
    }
}

/// List of system calls supported by the kernel.
#[derive(Debug, Clone)]
#[repr(C)]
#[non_exhaustive]
pub enum SystemCall {
    /// No system call. This should not be invoked.
    None,
    /// Yield system call. Doesn't need a capability.
    /// Used to give up the current timeslice.
    Yield,
    /// Print some string from the payload.
    Print,

    /// Given a caddr, get the total size and free size of the
    /// untyped capabilty space.
    UntypedTotalFree(CAddr),

    /// Create a new raw page capability using the provided
    /// untyped memory and store the capability in the current cpool.
    /// Returns the new CAddr.
    RawPageRetype { untyped_memory: CAddr },
    /// Map a given page into the provided address.
    RawPageMap {
        /// To map raw pages, we might need more pages for inner tables.
        untyped_memory: CAddr,
        /// The top level table into which the mapping should be done.
        top_level_table: CAddr,
        /// The address where the mapping should be done to.
        vaddr: u64,
        /// The raw page capability for the request.
        raw_page: CAddr,
    },
}

impl Default for SystemCall {
    fn default() -> Self {
        Self::None
    }
}

impl SystemCall {
    /// Convert the system call representation into a tuple so that
    /// it can be stored directly in registers instead of memory.
    pub fn as_regs(&self) -> Result<(u64, u64, u64, u64, u64), ()> {
        match self {
            SystemCall::Yield => Ok((1, 0, 0, 0, 0)),
            SystemCall::Print => Ok((2, 0, 0, 0, 0)),
            SystemCall::UntypedTotalFree(a) => Ok((3, a.into_u64(), 0, 0, 0)),
            SystemCall::RawPageRetype { untyped_memory: a } => Ok((100, a.into_u64(), 0, 0, 0)),
            SystemCall::RawPageMap {
                untyped_memory,
                top_level_table,
                vaddr,
                raw_page,
            } => Ok((
                101,
                untyped_memory.into_u64(),
                top_level_table.into_u64(),
                *vaddr,
                raw_page.into_u64(),
            )),
            _ => Err(()),
        }
    }

    /// Convert the in-register representtaion to the system call representation
    /// Reverse of [`Self::as_regs`].
    pub fn from_regs(a: u64, b: u64, c: u64, d: u64, e: u64) -> Result<SystemCall, ()> {
        match a {
            1 => Ok(SystemCall::Yield),
            2 => Ok(SystemCall::Print),
            3 => Ok(SystemCall::UntypedTotalFree(CAddr::from_u64(b))),
            100 => Ok(SystemCall::RawPageRetype {
                untyped_memory: CAddr::from_u64(b),
            }),
            101 => Ok(SystemCall::RawPageMap {
                untyped_memory: CAddr::from_u64(b),
                top_level_table: CAddr::from_u64(c),
                vaddr: d,
                raw_page: CAddr::from_u64(e),
            }),
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
