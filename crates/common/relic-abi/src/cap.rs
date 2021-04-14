use crate::SetDefault;

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

/// Represents a task buffer used for system calls.
pub struct TaskBuffer {
    pub call: Option<SystemCall>,
    pub payload_length: usize,
    pub payload_data: [u8; 1024],
}

impl SetDefault for TaskBuffer {
    fn set_default(&mut self) {
        self.call = None;
    }
}

#[derive(Debug, Clone)]
pub enum SystemCall {}
