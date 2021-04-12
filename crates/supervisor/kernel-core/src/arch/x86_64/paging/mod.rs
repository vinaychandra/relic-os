/// Representations of page tables.
pub mod table;

/// Utilities for paging
pub mod utils;

/// Basic page length in x86_64 (4 KiB).
pub const BASE_PAGE_LENGTH: usize = 4096; // 4 KiB

/// MAXPHYADDR, which is at most 52; (use CPUID for finding system value).
pub const MAXPHYADDR: u64 = 52;

/// Mask to find the physical address of an entry in a page-table.
const ADDRESS_MASK: u64 = ((1 << MAXPHYADDR) - 1) & !0xfff;
