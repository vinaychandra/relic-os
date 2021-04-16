use std::mem;

use relic_abi::cap::CapabilityErrors;
use relic_utils::align;
use spin::RwLock;

use crate::{
    addr::PAddrGlobal,
    util::managed_arc::{ManagedArc, ManagedArcAny},
};

/// Untyped memory descriptor. Represents a
/// chunk of physical memory.
#[derive(Getters, Debug)]
pub struct UntypedDescriptor {
    /// Start physical address of the untyped region.
    #[getset(get = "pub")]
    start_paddr: PAddrGlobal,
    /// Length of the untyped region.
    #[getset(get = "pub")]
    length: usize,
    watermark: PAddrGlobal,

    first_child: Option<ManagedArcAny>,
}
/// Untyped capability. Reference-counted smart pointer to untyped
/// descriptor.
///
/// Untyped capability represents free memory that can be retyped to
/// different useful capabilities.
pub type UntypedCap = ManagedArc<RwLock<UntypedDescriptor>>;

impl UntypedCap {
    /// Bootstrap an untyped capability using a memory region information.
    ///
    /// # Safety
    ///
    /// Can only be used for free memory regions returned from
    /// `InitInfo`.
    pub unsafe fn bootstrap(start_paddr: PAddrGlobal, length: usize) -> Self {
        let start_paddr_usize: usize = start_paddr.into();
        let des_paddr: PAddrGlobal =
            align::align_up(start_paddr_usize, UntypedCap::inner_type_alignment()).into();
        assert!(des_paddr + UntypedCap::inner_type_length() <= start_paddr + length);

        Self::new(
            des_paddr.into(),
            RwLock::new(UntypedDescriptor {
                start_paddr,
                length,
                watermark: des_paddr + UntypedCap::inner_type_length(),
                first_child: None,
            }),
        )
    }
}

impl UntypedDescriptor {
    /// Allocate a memory region using the given length and
    /// alignment. Shift the watermark of the current descriptor
    /// passing over the allocated region.
    pub unsafe fn allocate(
        &mut self,
        length: usize,
        alignment: usize,
    ) -> Result<PAddrGlobal, CapabilityErrors> {
        let paddr: PAddrGlobal = align::align_up(self.watermark.into(), alignment).into();
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
    pub unsafe fn derive<F>(
        &mut self,
        length: usize,
        alignment: usize,
        f: F,
    ) -> Result<(), CapabilityErrors>
    where
        F: FnOnce(PAddrGlobal, Option<ManagedArcAny>) -> ManagedArcAny,
    {
        let paddr = self.allocate(length, alignment)?;
        self.first_child = Some(f(paddr, self.first_child.take()));
        Ok(())
    }
}

impl Drop for UntypedDescriptor {
    fn drop(&mut self) {
        if let Some(child) = self.first_child.take() {
            mem::drop(child)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::mem::MaybeUninit;

    use super::*;

    #[test]
    fn test_untyped_memory() {
        let underlying_value: Box<MaybeUninit<[u64; 4096]>> = Box::new(MaybeUninit::uninit());
        let box_addr = Box::into_raw(underlying_value) as u64;
        let addr = PAddrGlobal::new(box_addr);

        let untyped_memory = unsafe { UntypedCap::bootstrap(addr, 4096) };
        {
            unsafe {
                untyped_memory
                    .write()
                    .derive(100, 4, |a, b| {
                        assert!(b.is_none());
                        let child = UntypedCap::bootstrap(a, 100);
                        child.write().first_child = b;
                        child
                    })
                    .unwrap();

                assert_eq!(
                    Err(CapabilityErrors::MemoryNotSufficient),
                    untyped_memory.write().derive(4000, 4, |_, _| {
                        unreachable!("Unexpected succesful allocated")
                    })
                );

                untyped_memory
                    .write()
                    .derive(100, 4, |a, b| {
                        assert!(b.is_some());
                        let child = UntypedCap::bootstrap(a, 100);
                        child.write().first_child = b;
                        child
                    })
                    .unwrap();
            }
        }
    }
}
