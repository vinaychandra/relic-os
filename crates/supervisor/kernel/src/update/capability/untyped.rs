use relic_abi::cap::CapabilityErrors;
use relic_utils::align;

use super::*;

pub struct UntypedMemoryRead<'a> {
    start_paddr: &'a PAddrGlobal,
    length: &'a Size,
    watermark: &'a PAddrGlobal,

    first_child_item: &'a Option<CapRcBoxed>,
}

pub struct UntypedMemoryWrite<'a> {
    start_paddr: &'a mut PAddrGlobal,
    length: &'a mut Size,
    watermark: &'a mut PAddrGlobal,

    first_child_item: &'a mut Option<CapRcBoxed>,
}

impl Capability {
    pub fn untyped_create(&self) -> Option<UntypedMemoryRead<'_>> {
        if let Capability::UntypedMemory {
            first_child_item,
            length,
            start_paddr,
            watermark,
            ..
        } = self
        {
            Some(UntypedMemoryRead {
                first_child_item,
                length,
                watermark,
                start_paddr,
            })
        } else {
            None
        }
    }

    pub fn untyped_create_mut(&mut self) -> Option<UntypedMemoryWrite<'_>> {
        if let Capability::UntypedMemory {
            first_child_item,
            length,
            start_paddr,
            watermark,
            ..
        } = self
        {
            Some(UntypedMemoryWrite {
                first_child_item,
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
    /// Can only be used for free memory regions returned from
    /// `InitInfo`.
    pub unsafe fn untyped_bootstrap(start_paddr: PAddrGlobal, length: usize) -> Self {
        Self::UntypedMemory {
            start_paddr,
            length,
            watermark: start_paddr,

            first_child_item: None,

            next: None,
            prev: None,
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
        F: FnOnce(*mut T, Option<CapRcBoxed>) -> Result<CapRcBoxed, CapabilityErrors>,
    {
        let length = core::mem::size_of::<T>();
        let alignment = core::mem::align_of::<T>();
        let paddr = self.allocate(length, alignment)?;

        let first_child = self.first_child_item.take();
        let f_success = f(unsafe { paddr.as_raw_ptr() }, first_child.clone());
        match f_success {
            Ok(new_child) => {
                *self.first_child_item = Some(new_child);
                Ok(())
            }
            Err(c) => {
                // F failed. Store the old item back.
                *self.first_child_item = first_child;
                Err(c)
            }
        }
    }
}
