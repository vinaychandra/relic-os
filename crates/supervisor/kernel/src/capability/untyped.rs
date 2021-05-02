/*!
Untyped memory support

Main memory data structure for the kernel. A single untyped memory capability
represents a contiguous area of memory. This doesn't occupy any space on the memory
itself but just represents an owner of memory segment.

Untyped memory allocates objects used by kernel or user. Deallocation is done only
when all children are removed.

All data contained in the untyped children is only accessible by the kernel. Only
data that is mapped using corresponding capabilities is accessible by the user.
*/
use relic_abi::cap::CapabilityErrors;
use relic_utils::align;

use super::*;

/// Untyped memory capability. Denotes a piece of physical memory
/// owned by the object.
/// See module level documentation for more details.
#[derive(Debug, CopyGetters)]
pub struct UntypedMemory {
    /**
    Starting physical address location of the untyped memory.
    */
    start_paddr: PAddrGlobal,
    /**
    The length of the physical memory owned by this capability.
    */
    #[getset(get_copy = "pub")]
    length: usize,
    /**
    The position in physical memory until which memory has been
    allocated. This is reset to `start_paddr` only when the
    `child_mem_item` becomes empty.
    */
    watermark: PAddrGlobal,

    /**
    The first child object in the untyped capabilities memory
    tree. Represents a tree of objects which are contained in this
    capability.
    */
    child_mem_item: Option<StoredCap>,
}

impl UntypedMemory {
    /**
    Bootstrap an untyped capability using a memory region information.

    # Safety
    Can only be used for free memory regions returned from bootstrap.
    */
    pub unsafe fn bootstrap(start_paddr: PAddrGlobal, length: usize) -> Capability {
        let data = UntypedMemory {
            start_paddr,
            length,
            watermark: start_paddr,
            child_mem_item: None,
        };
        Capability {
            capability_data: CapabilityEnum::UntypedMemory(data),
            ..Default::default()
        }
    }

    /**
    Get free space in bytes.
    */
    pub fn get_free_space(&self) -> u64 {
        self.start_paddr + self.length - self.watermark
    }

    /**
    Allocate a memory region using the given length and
    alignment. Shift the watermark of the current descriptor
    passing over the allocated region.
    */
    pub fn allocate(
        &mut self,
        length: usize,
        alignment: usize,
    ) -> Result<PAddrGlobal, CapabilityErrors> {
        let paddr: PAddrGlobal = align::align_up((self.watermark).into(), alignment).into();
        if paddr + length > self.start_paddr + self.length {
            return Err(CapabilityErrors::MemoryNotSufficient);
        }

        self.watermark = paddr + length;
        Ok(paddr)
    }

    /**
    Check whether the provided length and alignment can be allocated in the current region.
    */
    pub fn can_allocate(&self, length: usize, alignment: usize) -> bool {
        let paddr: PAddrGlobal = align::align_up((self.watermark).into(), alignment).into();
        paddr + length <= self.start_paddr + self.length
    }

    /**
    Derive and allocate a memory region to a capability that
    requires memory region.
    The provided function is given the new pointer to store the new
    derived data. The function should return the new derived data
    to store as the next item in the derivation tree.
    The alignment of the data created is taken from the alignment parameter.
    If None, the alignemnt is the same as that of type parameter `T`.
    */
    pub fn derive<T, F>(
        &mut self,
        alignment: Option<usize>,
        f: F,
    ) -> Result<StoredCap, CapabilityErrors>
    where
        F: FnOnce(*mut T) -> Result<StoredCap, CapabilityErrors>,
    {
        let length = core::mem::size_of::<T>();
        let alignment = if let Some(align_val) = alignment {
            align_val
        } else {
            core::mem::align_of::<T>()
        };
        let paddr = self.allocate(length, alignment)?;

        let f_success = f(unsafe { paddr.as_raw_ptr() })?;

        {
            let mut fs_write = f_success.borrow_mut();

            let to_be_second = self.child_mem_item.take();
            if let Some(ref sec) = to_be_second {
                let mut sec_write = sec.as_ptr();
                unsafe { (*sec_write).prev_mem_item = Some(f_success.clone()) };
            }

            fs_write.next_mem_item = to_be_second;
            self.child_mem_item = Some(f_success.clone());
        }

        Ok(f_success)
    }
}
