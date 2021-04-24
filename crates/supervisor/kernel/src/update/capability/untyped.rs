use relic_abi::cap::CapabilityErrors;
use relic_utils::align;

use super::*;

#[derive(Debug)]
pub struct UntypedMemory {
    pub start_paddr: PAddrGlobal,
    pub length: Size,
    pub watermark: PAddrGlobal,

    pub children: LinkedList<MemTreeAdapter>,
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
            children: LinkedList::new(MemTreeAdapter::new()),
        };
        Self {
            capability_data: RefCell::new(CapabilityEnum::UntypedMemory(data)),
            mem_tree_link: LinkedListLink::new(),
            paging_tree_link: LinkedListLink::new(),
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

    /// Derive and allocate a memory region to a capability that
    /// requires memory region.
    /// The provided function is given the new PAddr to store the new
    /// derived data and an optional next child in the derivation tree.
    /// The function should return the new derived data to store as the
    /// next item in the derivation tree.
    pub fn derive<T, F>(&mut self, f: F) -> Result<UnsafeRef<Capability>, CapabilityErrors>
    where
        F: FnOnce(*mut T) -> Result<UnsafeRef<Capability>, CapabilityErrors>,
    {
        let length = core::mem::size_of::<T>();
        let alignment = core::mem::align_of::<T>();
        let paddr = self.allocate(length, alignment)?;

        let f_success = f(unsafe { paddr.as_raw_ptr() })?;
        self.children.push_front(f_success.clone());
        Ok(f_success)
    }
}
