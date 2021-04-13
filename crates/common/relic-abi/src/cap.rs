/// Errors when using capabilities.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum CapabilityErrors {
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
}
