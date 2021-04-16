use core::convert::TryFrom;

/// Errors when using capabilities.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u64)]
#[non_exhaustive]
pub enum CapabilityErrors {
    None = 0,
    /// This capability slot is already occupied.
    CapabilityAlreadyOccupied,
    /// There are no free capabilities to store.
    CapabilitySlotsFull,
    /// A search for a capability or an empty slot failed.
    CapabilitySearchFailed,

    /// This memory is already mapped.
    MemoryAlreadyMapped,
    /// Out of memory error.
    MemoryNotSufficient,

    /// Unknown syscall.
    SyscallNotFound,

    /// Unknown cap error.
    Unknown, // Keep this last.
}

impl CapabilityErrors {
    pub fn to_u64(&self) -> u64 {
        *self as u64
    }
}

impl TryFrom<u64> for CapabilityErrors {
    type Error = ();

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        if value > CapabilityErrors::Unknown as u64 {
            return Err(());
        }

        let result: CapabilityErrors = unsafe { core::mem::transmute(value) };
        Ok(result)
    }
}
