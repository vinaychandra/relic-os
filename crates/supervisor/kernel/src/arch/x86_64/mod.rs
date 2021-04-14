/// Bootstrap logic for architecture
pub mod bootstrap;

/// Global Descriptor Table.
pub mod gdt;

/// Arch level configuration.
pub mod globals;

/// Interrupt support.
pub mod interrupts;

/// Paging implementation for the kernel.
pub mod paging;

/// Serial port controller.
pub mod serial;

pub mod tls;

use crate::arch::serial::SerialLogger;

/// Logger that uses serial to output logs.
/// Architecture level logs for x86_64.
pub static LOGGER: SerialLogger = SerialLogger;

#[global_allocator]
static A: static_alloc::Bump<[u8; 1 << 16]> = static_alloc::Bump::uninit(); // 64KB

pub mod cpu_locals {
    use core::cell::Cell;

    pub use super::interrupts::apic::LAPIC;
    pub use super::interrupts::apic::PROCESSOR_ID;

    #[thread_local]
    pub static CURRENT_THREAD_ID: Cell<usize> = Cell::new(0);
}
