use log::Level;

/// Log settings
pub const DEFAULT_LOG_LEVEL: Level = Level::Info;
pub const EXTRA_LOGS: [&'static str; 1] = ["bootstrap"];

/// Size of stack used as an intermediate stack when bootstrapping the system.
/// This stack is hardcoded as an array in the binary.
pub const BSP_TEMP_STACK_SIZE_BYTES: usize = 4096 * 4;

/// Bytes for stack alignment offset.
pub const STACK_ALIGN: usize = 128;

/// The location where all of memory is mapped to.
pub const MEM_MAP_OFFSET_LOCATION: u64 = 0xFFFF_FF00_0000_0000;
pub const MEM_MAP_SIZE: u64 = 512 * 1024 * 1024 * 1024;

/// Kernel uses 2 MiB pages. Number of pages for each kernel stack.
pub const KERNEL_STACK_NUM_PAGES: usize = 2;

/// Start location of kernel stacks.
/// First stack is from [`KERNEL_STACK_START`] to
/// `KERNEL_STACK_START  + KERNEL_STACK_NUM_PAGES * 2MiB`.
pub const KERNEL_STACK_START: usize = 0xFFFF_FF80_0000_0000;

/// Basic page length in x86_64 (4 KiB).
pub const BASE_PAGE_LENGTH: usize = 4096; // 4 KiB
