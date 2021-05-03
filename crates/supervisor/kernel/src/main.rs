//! Relis OS - Kernel code.

#![cfg_attr(not(test), no_std)]
#![cfg_attr(test, allow(unused_imports))]
#![cfg_attr(test, feature(new_uninit))]
#![cfg_attr(not(test), no_main)]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]
#![feature(asm)]
#![feature(assert_matches)]
#![feature(coerce_unsized)]
#![feature(const_fn)]
#![feature(const_raw_ptr_to_usize_cast)]
#![feature(dispatch_from_dyn)]
#![feature(maybe_uninit_extra)]
#![feature(naked_functions)]
#![feature(option_get_or_insert_default)]
#![feature(result_flattening)]
#![feature(thread_local)]
#![feature(trace_macros)]
#![feature(type_ascription)]
#![feature(unsize)]

use crate::{
    addr::{PAddrGlobal, VAddr},
    arch::globals::{self, BASE_PAGE_LENGTH},
    capability::{
        CapAccessorMut, Capability, CapabilityEnum, Cpool, CpoolInner, MapPermissions, Scheduler,
        StoredCap, UntypedMemory,
    },
    logging::UnifiedLogger,
    ramdisk::{elf_loader::DefaultElfLoader, ustar::UStarArchive},
    relic_utils::align,
    util::{boxed::Boxed, memory_region::MemoryRegion},
};
use elfloader::ElfBinary;
use heapless::Vec;
use relic_abi::{bootstrap::BootstrapInfo, syscall::TaskBuffer};
use std::{cell::RefCell, panic::PanicInfo};

extern crate alloc;
extern crate core as std;

#[allow(unused_imports)]
#[macro_use]
extern crate relic_utils;

#[macro_use]
extern crate bitflags;

#[macro_use]
extern crate bitfield;

#[macro_use]
extern crate getset;

#[macro_use]
extern crate log;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate paste;

#[macro_use]
extern crate static_assertions;

pub mod arch;
pub mod frame_buffer;
pub mod logging;

/// Support for loading of Sigma process.
pub mod ramdisk;

/// Support for addresses.
pub mod addr;

/// Utilities for the kernel.
pub mod util;

pub mod capability;

/// Logic to process syscalls.
pub mod syscall_processor;

// BOOTBOOT is autogenerated. So, we ignore a bunch of warnings.
#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(non_camel_case_types)]
#[allow(unaligned_references)]
#[allow(deref_nullptr)]
mod bootboot;

/// Logger used by the kernel everywhere. This logger is activated by the architecture
/// level startup once the memory is ready.
pub static KERNEL_LOGGER: UnifiedLogger = UnifiedLogger::new();

/// Entry point for the Operating System. This calls into the bootstrap
/// of architecture.
#[cfg(not(test))]
#[no_mangle]
fn _start() -> ! {
    crate::arch::bootstrap::initialize_bootstrap_core()
}

/// This function is called on panic.
#[cfg_attr(target_os = "none", panic_handler)]
fn _panic_handler(info: &PanicInfo) -> ! {
    info!("====== KERNEL_PANIC ======");
    error!("Panic: {}", info);
    info!("====== KERNEL_PANIC ======");
    loop {}
}

#[cfg_attr(target_os = "none", alloc_error_handler)]
fn _alloc_error_handler(layout: alloc::alloc::Layout) -> ! {
    panic!("allocation error: {:?}", layout)
}

/// Main Function on bootstrap processor.
/// This function should not return.
pub fn main_bsp(free_regions: Vec<MemoryRegion, 32>) -> ! {
    info!(target: "main", "Free regions found: {:?}", free_regions);

    let mut bootstrap_info = BootstrapInfo {
        top_level_pml4: 0.into(),
        free_mem_regions: (0.into(), 0.into()),
        ..Default::default()
    };
    const NONE_INNER: RefCell<Capability> = RefCell::new(Capability::new());
    let root_cpool_inner = CpoolInner {
        unsafe_data: [NONE_INNER; 256],
    };
    let mut root_cpool = Cpool {
        linked_task: None,
        data: unsafe {
            Boxed::new(PAddrGlobal::new(
                &root_cpool_inner as *const CpoolInner as u64,
            ))
        },
    };

    let mut largest_index = usize::MAX;
    let mut largest_size = 0;
    for (index, region) in free_regions.iter().enumerate() {
        if region.length() > largest_size {
            largest_size = region.length();
            largest_index = index;
        }
        let untyped = unsafe {
            UntypedMemory::bootstrap(
                region.start_paddr().to_paddr_global(),
                region.length(),
                false,
            )
        };
        root_cpool
            .write_to_if_empty(index, untyped)
            .expect("Failed to create initial memory mappings.");
    }
    bootstrap_info.free_mem_regions.1 = (free_regions.len() as u8 - 1).into();

    let cpool_cap = Capability {
        capability_data: CapabilityEnum::Cpool(root_cpool),
        ..Default::default()
    };

    let root_cpool_refcell = RefCell::new(cpool_cap);
    let root_cpool_stored: StoredCap = (&root_cpool_refcell).into();

    let root_cpool = root_cpool_stored.as_cpool_mut().unwrap();
    let untyped = root_cpool.lookup((largest_index as u8).into()).unwrap();
    let (task_cap, _pml4, _root_cpool) = load_sigma(
        root_cpool,
        untyped.as_untyped_memory_mut().unwrap(),
        bootstrap_info,
    );

    let scheduler = Scheduler::new();
    scheduler.add_task_with_priority(&mut task_cap.as_task_mut().unwrap());
    scheduler.run_forever()
}

// Return task, pml4
fn load_sigma(
    cpool_cap: CapAccessorMut<'_, Cpool>,
    untyped_cap: CapAccessorMut<'_, UntypedMemory>,
    mut bootstrap_info: BootstrapInfo,
) -> (StoredCap, StoredCap, StoredCap) {
    let ramdisk: UStarArchive;
    let binary = {
        unsafe {
            let initrd_ptr =
                (bootboot::bootboot.initrd_ptr + globals::MEM_MAP_OFFSET_LOCATION) as *const u8;
            ramdisk = UStarArchive::new(initrd_ptr, bootboot::bootboot.initrd_size as usize);
            info!(target: "load_sigma", "Initrd image is {}", ramdisk);

            let file_name = "./userspace/relic-sigma";
            let file = ramdisk.lookup(file_name).expect("Sigma file not found");
            let binary = ElfBinary::new("relic-sigma", file).expect("Cannot read the binary");
            binary
        }
    };
    let mut loader =
        DefaultElfLoader::new(VAddr::new(0), cpool_cap, &mut bootstrap_info, untyped_cap);
    binary.load(&mut loader).expect("Binary loading failed");
    let loc: u64 = loader.exe_section_location().into();
    info!(target: "load_sigma",
            "Sigma project loaded. Use comand `add-symbol-file ../../x86_64-relic-user/debug/relic-sigma  0x{:x}`",
            loc);
    let (mut untyped_cap, mut cpool_cap, pml4_cap_stored) = loader.unwrap();
    let mut pml4_cap = pml4_cap_stored.as_l4_mut().unwrap();
    let pml4 = pml4_cap.cap().clone();

    info!(target: "load_sigma", "Loading kernel stack");
    let user_stack_start: u64 = 0x6FFF_000_0000;
    let num_pages = 10usize;
    for page_index in 0..num_pages {
        DefaultElfLoader::map_empty_page(
            &mut pml4_cap,
            &mut untyped_cap,
            &mut cpool_cap,
            VAddr::new(user_stack_start + (BASE_PAGE_LENGTH * page_index) as u64),
            MapPermissions::WRITE,
        )
    }
    let user_stack_end: VAddr = align::align_down(
        user_stack_start + num_pages as u64 * BASE_PAGE_LENGTH as u64 - 1,
        globals::STACK_ALIGN as u64,
    )
    .into();
    info!(target:"load_sigma", "Stack loaded at {:?}", user_stack_end);

    {
        info!(target:"load_sigma", "Load VGA Buffers");
        let bootstrap = frame_buffer::FrameBuffer::new_from_bootboot();
        bootstrap.bootstrap_and_map(
            &mut untyped_cap,
            &mut cpool_cap,
            &mut pml4_cap,
            &mut bootstrap_info,
        );
    }

    info!(target: "load_sigma", "Loading TaskBuffers");
    let buffer_start: u64 = 0x6000_000_0000;
    let (buffer_cap, ind) =
        StoredCap::base_page_retype_from::<TaskBuffer>(&mut untyped_cap, &mut cpool_cap, true)
            .unwrap();
    info!(target: "load_sigma", "TaskBufferCap is stored at index {}", ind);
    pml4_cap
        .l4_map(
            buffer_start.into(),
            &buffer_cap,
            &mut untyped_cap,
            &mut cpool_cap,
            None,
            MapPermissions::WRITE,
        )
        .unwrap();

    let mut buffer = buffer_cap.as_base_page_mut().unwrap();
    buffer.page_data_mut::<TaskBuffer>().self_address = buffer_start;

    // Load bootstrap info into payload
    buffer
        .page_data_mut::<TaskBuffer>()
        .write_to_task_buffer(&bootstrap_info)
        .unwrap();

    let (task_cap, _task_cap_index) =
        StoredCap::task_retype_from(&mut untyped_cap, &mut cpool_cap, 15).unwrap();
    let mut task_cap_write = task_cap.as_task_mut().unwrap();

    task_cap_write.set_instruction_pointer(binary.entry_point().into());
    task_cap_write.set_stack_pointer(user_stack_end);
    task_cap_write.set_status(capability::TaskStatus::Inactive);

    task_cap_write.set_tcb_location(buffer_start.into());

    task_cap_write.task_set_cpool(&mut cpool_cap).unwrap();
    task_cap_write
        .task_set_top_level_table(&mut pml4_cap)
        .unwrap();
    task_cap_write.task_set_task_buffer(&mut buffer).unwrap();

    info!(target: "load_sigma", "Sigma task Cap: {:?}", task_cap);

    core::mem::drop(task_cap_write);
    (task_cap, pml4, cpool_cap.cap().clone())
}
