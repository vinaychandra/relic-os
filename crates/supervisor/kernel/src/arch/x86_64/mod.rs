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

pub mod capability;

/// Serial port controller.
pub mod serial;

/// Runtime for threads.
pub mod task;

pub mod tls;

use relic_abi::cap::CapabilityErrors;

use crate::{
    addr::{PAddr, PAddrGlobal, VAddr},
    arch::{
        paging::table::{pd_index, pdpt_index, pml4_index, pt_index, PD, PDPT, PML4, PT},
        serial::SerialLogger,
    },
};

/// Logger that uses serial to output logs.
/// Architecture level logs for x86_64.
pub static LOGGER: SerialLogger = SerialLogger;

#[cfg(test)]
const MEM_SIZE: usize = 1 << 25;
#[cfg(not(test))]
const MEM_SIZE: usize = 1 << 16;

#[global_allocator]
static GLOBAL_ALLOC: static_alloc::Bump<[u8; MEM_SIZE]> = static_alloc::Bump::uninit(); // 64KB

pub mod cpu_locals {
    pub use super::interrupts::apic::LAPIC;
    pub use super::interrupts::apic::PROCESSOR_ID;
}

impl PAddrGlobal {
    pub fn assert_in_good_range(self) {
        let val: u64 = self.into();
        const OFFSET_END: u64 = globals::MEM_MAP_OFFSET_LOCATION + globals::MEM_MAP_SIZE;
        debug_assert!(val > globals::MEM_MAP_OFFSET_LOCATION && val < OFFSET_END);
    }
}

impl VAddr {
    pub fn validate_kernel_mode(self) -> Result<(), CapabilityErrors> {
        let val: u64 = self.into();
        let val = val >> 47;
        if val == 0b1111_1111_1111_1111_1 {
            Ok(())
        } else {
            Err(CapabilityErrors::InvalidMemoryAddress)
        }
    }

    pub fn validate_user_mode(self) -> Result<(), CapabilityErrors> {
        let val: u64 = self.into();
        let val = val >> 47;
        if val == 0 {
            Ok(())
        } else {
            Err(CapabilityErrors::InvalidMemoryAddress)
        }
    }

    /// Translate a vaddr to paddr in given level4 page.
    pub fn translate(self, l4: &PML4) -> Option<PAddr> {
        let addr_mapping = |addr: PAddr| {
            let value: u64 = addr.into();
            VAddr::new(value + globals::MEM_MAP_OFFSET_LOCATION)
        };

        unsafe {
            let l3_entry = l4[pml4_index(self)];
            if !l3_entry.is_present() {
                None?
            }
            let l3_paddr = l3_entry.get_address();
            let l3_vaddr = addr_mapping(l3_paddr);
            let l3: &PDPT = l3_vaddr.as_mut_ptr();
            let l2_entry = l3[pdpt_index(self)];
            if !l2_entry.is_present() {
                None?
            }
            let l2_paddr = l2_entry.get_address();
            let l2_vaddr = addr_mapping(l2_paddr);
            let l2: &PD = l2_vaddr.as_mut_ptr();
            let l1_entry = l2[pd_index(self)];
            if !l1_entry.is_present() {
                None?
            }
            let l1_paddr = l1_entry.get_address();
            let l1_vaddr = addr_mapping(l1_paddr);
            let l1: &PT = l1_vaddr.as_mut_ptr();
            let l0_entry = l1[pt_index(self)];
            if !l0_entry.is_present() {
                None?
            }
            let l0_paddr = l0_entry.get_address();

            let vaddr_u64: u64 = self.into();
            let page_paddr_u64: u64 = l0_paddr.into();

            Some((page_paddr_u64 | (vaddr_u64 & 0b111111111111)).into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_vaddr_location() {
        assert!(VAddr::new(0x0).validate_user_mode().is_ok());
        assert!(VAddr::new(0x7FFF_FFFF_FFFF).validate_user_mode().is_ok());
        assert!(VAddr::new(0xFFFF_8FFF_FFFF_FFFF)
            .validate_kernel_mode()
            .is_ok());
        assert!(VAddr::new(0xFFFF_FFFF_FFFF_FFFF)
            .validate_kernel_mode()
            .is_ok());
    }
}
