use relic_abi::cap::CapabilityErrors;
use relic_utils::align;

use super::*;

#[derive(Debug)]
pub struct UntypedMemory {
    start_paddr: PAddrGlobal,
    length: Size,
    watermark: PAddrGlobal,

    child_mem_item: Option<StoredCap>,
}

impl Capability {
    /// Bootstrap an untyped capability using a memory region information.
    ///
    /// # Safety
    ///
    /// Can only be used for free memory regions returned from bootstrap.
    pub unsafe fn untyped_bootstrap(start_paddr: PAddrGlobal, length: usize) -> Self {
        let data = UntypedMemory {
            start_paddr,
            length,
            watermark: start_paddr,
            child_mem_item: None,
        };
        Self {
            capability_data: CapabilityEnum::UntypedMemory(data),
            ..Default::default()
        }
    }
}

impl UntypedMemory {
    /// Get free space in bytes.
    pub fn get_free_space(&self) -> Size {
        let len: u64 = (self.start_paddr + self.length - self.watermark).into();
        len as usize
    }

    /// Allocate a memory region using the given length and
    /// alignment. Shift the watermark of the current descriptor
    /// passing over the allocated region.
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

    pub fn can_allocate(&self, length: usize, alignment: usize) -> bool {
        let paddr: PAddrGlobal = align::align_up((self.watermark).into(), alignment).into();
        paddr + length <= self.start_paddr + self.length
    }

    /// Derive and allocate a memory region to a capability that
    /// requires memory region.
    /// The provided function is given the new PAddr to store the new
    /// derived data and an optional next child in the derivation tree.
    /// The function should return the new derived data to store as the
    /// next item in the derivation tree.
    pub fn derive<T, F>(&mut self, f: F) -> Result<StoredCap, CapabilityErrors>
    where
        F: FnOnce(*mut T) -> Result<StoredCap, CapabilityErrors>,
    {
        let length = core::mem::size_of::<T>();
        let alignment = core::mem::align_of::<T>();
        let paddr = self.allocate(length, alignment)?;

        let f_success = f(unsafe { paddr.as_raw_ptr() })?;

        {
            let mut fs_write = f_success.borrow_mut();

            let to_be_second = self.child_mem_item.take();
            if let Some(ref sec) = to_be_second {
                let mut sec_write = sec.borrow_mut();
                sec_write.prev_mem_item = Some(f_success.clone());
            }

            fs_write.next_mem_item = to_be_second;
            self.child_mem_item = Some(f_success.clone());
        }

        Ok(f_success)
    }
}
