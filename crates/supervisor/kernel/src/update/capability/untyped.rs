use relic_abi::cap::CapabilityErrors;
use relic_utils::align;

use super::*;

impl Capability {
    /// Bootstrap an untyped capability using a memory region information.
    ///
    /// # Safety
    ///
    /// Can only be used for free memory regions returned from bootstrap.
    pub unsafe fn untyped_bootstrap(start_paddr: PAddrGlobal, length: usize) -> Self {
        let data = CapabilityEnum::UntypedMemory {
            start_paddr,
            length,
            watermark: start_paddr,
            children: LinkedList::new(MemTreeAdapter::new()),
        };
        Self {
            capability_data: RefCell::new(data),
            mem_tree_link: LinkedListLink::new(),
            paging_tree_link: LinkedListLink::new(),
        }
    }

    /// Get free space in bytes.
    pub fn untyped_get_free_space(&self) -> Result<Size, CapabilityErrors> {
        if let CapabilityEnum::UntypedMemory {
            start_paddr,
            length,
            watermark,
            ..
        } = &*self.capability_data.borrow()
        {
            let len: u64 = (*start_paddr + *length - *watermark).into();
            Ok(len as usize)
        } else {
            Err(CapabilityErrors::CapabilityMismatch)
        }
    }

    /// Allocate a memory region using the given length and
    /// alignment. Shift the watermark of the current descriptor
    /// passing over the allocated region.
    pub fn untyped_allocate(
        &self,
        length: usize,
        alignment: usize,
    ) -> Result<PAddrGlobal, CapabilityErrors> {
        if let CapabilityEnum::UntypedMemory {
            start_paddr,
            length: mem_length,
            watermark,
            ..
        } = &mut *self.capability_data.borrow_mut()
        {
            let paddr: PAddrGlobal = align::align_up((*watermark).into(), alignment).into();
            if paddr + length > *start_paddr + *mem_length {
                return Err(CapabilityErrors::MemoryNotSufficient);
            }

            *watermark = paddr + length;
            Ok(paddr)
        } else {
            Err(CapabilityErrors::CapabilityMismatch)
        }
    }

    /// Derive and allocate a memory region to a capability that
    /// requires memory region.
    /// The provided function is given the new PAddr to store the new
    /// derived data and an optional next child in the derivation tree.
    /// The function should return the new derived data to store as the
    /// next item in the derivation tree.
    pub fn untyped_derive<T, F>(&self, f: F) -> Result<(), CapabilityErrors>
    where
        F: FnOnce(*mut T) -> Result<UnsafeRef<Capability>, CapabilityErrors>,
    {
        let length = core::mem::size_of::<T>();
        let alignment = core::mem::align_of::<T>();
        let paddr = self.untyped_allocate(length, alignment)?;

        if let CapabilityEnum::UntypedMemory { children, .. } =
            &mut *self.capability_data.borrow_mut()
        {
            let f_success = f(unsafe { paddr.as_raw_ptr() })?;
            children.push_front(f_success);
            Ok(())
        } else {
            Err(CapabilityErrors::CapabilityMismatch)
        }
    }
}
