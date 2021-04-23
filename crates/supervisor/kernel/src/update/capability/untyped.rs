use relic_abi::cap::CapabilityErrors;
use relic_utils::align;

use super::*;

pub struct UntypedMemoryRead<'a> {
    start_paddr: &'a PAddrGlobal,
    length: &'a Size,
    watermark: &'a PAddrGlobal,

    #[allow(dead_code)]
    children: &'a LinkedList<MemTreeAdapter>,
}

pub struct UntypedMemoryWrite<'a> {
    start_paddr: &'a mut PAddrGlobal,
    length: &'a mut Size,
    watermark: &'a mut PAddrGlobal,

    children: &'a mut LinkedList<MemTreeAdapter>,
}

impl<'a> UntypedMemoryWrite<'a> {
    pub fn read(&self) -> UntypedMemoryRead<'_> {
        UntypedMemoryRead {
            children: self.children,
            length: self.length,
            watermark: self.watermark,
            start_paddr: self.start_paddr,
        }
    }
}

impl Capability {
    pub fn untyped_create(&self) -> Option<UntypedMemoryRead<'_>> {
        if let CapabilityEnum::UntypedMemory {
            children,
            length,
            start_paddr,
            watermark,
            ..
        } = &self.capability_data
        {
            Some(UntypedMemoryRead {
                children,
                length,
                watermark,
                start_paddr,
            })
        } else {
            None
        }
    }

    pub fn untyped_create_mut(&mut self) -> Option<UntypedMemoryWrite<'_>> {
        if let CapabilityEnum::UntypedMemory {
            children,
            length,
            start_paddr,
            watermark,
            ..
        } = &mut self.capability_data
        {
            Some(UntypedMemoryWrite {
                children,
                length,
                watermark,
                start_paddr,
            })
        } else {
            None
        }
    }

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
            capability_data: data,
            mem_tree_link: LinkedListLink::new(),
            paging_tree_link: LinkedListLink::new(),
        }
    }
}

impl<'a> UntypedMemoryRead<'a> {
    /// Get free space in bytes.
    pub fn untyped_get_free_space(&self) -> Size {
        let len: u64 = (*self.start_paddr + *self.length - *self.watermark).into();
        len as usize
    }
}

impl<'a> UntypedMemoryWrite<'a> {
    /// Allocate a memory region using the given length and
    /// alignment. Shift the watermark of the current descriptor
    /// passing over the allocated region.
    pub fn allocate(
        &mut self,
        length: usize,
        alignment: usize,
    ) -> Result<PAddrGlobal, CapabilityErrors> {
        let paddr: PAddrGlobal = align::align_up((*self.watermark).into(), alignment).into();
        if paddr + length > *self.start_paddr + *self.length {
            return Err(CapabilityErrors::MemoryNotSufficient);
        }

        *self.watermark = paddr + length;
        Ok(paddr)
    }

    /// Derive and allocate a memory region to a capability that
    /// requires memory region.
    /// The provided function is given the new PAddr to store the new
    /// derived data and an optional next child in the derivation tree.
    /// The function should return the new derived data to store as the
    /// next item in the derivation tree.
    pub fn derive<T, F>(&mut self, f: F) -> Result<(), CapabilityErrors>
    where
        F: FnOnce(*mut T) -> Result<UnsafeRef<Capability>, CapabilityErrors>,
    {
        let length = core::mem::size_of::<T>();
        let alignment = core::mem::align_of::<T>();
        let paddr = self.allocate(length, alignment)?;

        let f_success = f(unsafe { paddr.as_raw_ptr() })?;
        self.children.push_front(f_success);
        Ok(())
    }
}
