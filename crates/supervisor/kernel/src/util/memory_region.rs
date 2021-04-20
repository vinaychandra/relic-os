use relic_utils::align;

use crate::addr::PAddr;

/// Represents a memory region with a start physical address and a
/// length.
#[derive(Debug, Copy, Clone)]
pub struct MemoryRegion {
    start_paddr: PAddr,
    length: usize,
}

impl MemoryRegion {
    /// Start address of the memory region.
    pub fn start_paddr(&self) -> PAddr {
        self.start_paddr
    }

    /// Test whether the provided address is contained in this memory region or not.
    pub fn contains(&self, other: PAddr) -> bool {
        self.start_paddr <= other && self.end_paddr() >= other
    }

    /// Length of the memory region.
    pub fn length(&self) -> usize {
        self.length
    }

    /// End address of the memory region. (Inclusive)
    pub fn end_paddr(&self) -> PAddr {
        self.start_paddr + (self.length - 1)
    }

    /// Modify the current memory region so that it skip up to the
    /// argument `region`.
    pub fn skip_up(&mut self, region: &MemoryRegion) -> bool {
        if self.start_paddr() <= region.start_paddr() && self.end_paddr() >= region.end_paddr() {
            self.move_up(region.start_paddr() + region.length());

            true
        } else {
            false
        }
    }

    /// Modify the current memory region so that it move to the
    /// beginning of `npaddr`.
    pub fn move_up(&mut self, npaddr: PAddr) {
        assert!(npaddr >= self.start_paddr);
        assert!(self.start_paddr + self.length > npaddr);
        let nlength =
            Into::<usize>::into(self.start_paddr + self.length) - Into::<usize>::into(npaddr);
        self.length = nlength;
        self.start_paddr = npaddr;
    }

    /// Create a new memory region using `start_paddr` and `length`.
    pub fn new(start_paddr: PAddr, length: usize) -> MemoryRegion {
        MemoryRegion {
            start_paddr,
            length,
        }
    }

    /// Allocate a memory region using the given length and
    /// alignment. Shift the watermark of the current descriptor
    /// passing over the allocated region.
    pub fn try_allocate(&mut self, length: usize, alignment: usize) -> Option<PAddr> {
        let paddr: PAddr = align::align_up(self.start_paddr.into(), alignment).into();
        if paddr + length > self.start_paddr + self.length {
            return None;
        }

        let new_addr = paddr + length;
        self.move_up(new_addr);
        Some(paddr)
    }
}
