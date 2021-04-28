use core::convert::TryFrom;

/// Errors when using capabilities and syscalls.
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
    /// The requested capability and provided capabilities mismatch.
    CapabilityMismatch,

    /// This memory is already mapped.
    MemoryAlreadyMapped,
    /// Out of memory error.
    MemoryNotSufficient,
    /// Alignment for memory is unexpected.
    MemoryAlignmentFailure,
    /// The passed memory address is invalid.
    InvalidMemoryAddress,

    /// Unknown syscall.
    SyscallNotFound,

    /// Task buffer doesn't exist.
    TaskBufferNotFound,

    /// Unknown cap error.
    Unknown, // Keep this last, otherwise TryFrom will fail.
}

impl CapabilityErrors {
    /// Get the u64 representation of the error.
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
