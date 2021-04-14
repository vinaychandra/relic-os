use heapless::Vec;
use log::LevelFilter;
use relic_kernel_core::{
    arch::paging::{table::*, utils},
    prelude::*,
    util::memory_region::MemoryRegion,
};
use relic_utils::align;
use x86_64::registers::{control::Cr4Flags, model_specific::EferFlags};

use crate::arch::globals;

static BSP_STACK: [u8; globals::BSP_TEMP_STACK_SIZE_BYTES] =
    [0; globals::BSP_TEMP_STACK_SIZE_BYTES];

#[repr(align(4096))]
struct MemMapEntries([PDPTEntry; 512]);
/// Stack used to map 512GiB of VMem.
static mut MEM_MAP_STACK: MemMapEntries = MemMapEntries([PDPTEntry::empty(); 512]);

#[repr(align(4096))]
struct KernelPDEntries([PDEntry; 512]);
/// Stack used to map 512GiB of VMem.
static mut KERNEL_STACK_PD_ENTRIES: KernelPDEntries = KernelPDEntries([PDEntry::empty(); 512]);

pub fn initialize_bootstrap_core() -> ! {
    // Pages for initial bootstrapping. This acts as an intermediate step.
    // We need this for setting up for the main stacks but the bootloader only provdes 1K in memory.
    let bsp_addr = &BSP_STACK[0] as *const u8 as usize;
    let level_2_addr = align::align_down(
        (bsp_addr + BSP_STACK.len()) as u64,
        globals::STACK_ALIGN as u64,
    );

    // Switch to level 2.
    unsafe {
        asm!("
            mov rsp, {0}
            mov rbp, {0}
            jmp {1}
            ", in(reg) level_2_addr, sym initialize_bootstrap_core2, options(noreturn));
    }
}

/// Level 2 initializing.
/// This creates a memory map in higher half and then jumps to it.
fn initialize_bootstrap_core2() -> ! {
    // Intialize logging
    log::set_logger(&crate::KERNEL_LOGGER)
        .map(|()| log::set_max_level(LevelFilter::Trace))
        .expect("Setting logger failed");

    // This enables syscall extensions on x86_64
    {
        let mut efer = x86_64::registers::model_specific::Efer::read();
        efer |= EferFlags::NO_EXECUTE_ENABLE;
        efer |= EferFlags::SYSTEM_CALL_EXTENSIONS;
        unsafe {
            x86_64::registers::model_specific::Efer::write(efer);
        }
    }

    {
        let mut cr4 = x86_64::registers::control::Cr4::read();
        cr4 |= Cr4Flags::PAGE_GLOBAL;
        cr4 |= Cr4Flags::PCID;
    }

    let current_page_table: &mut PML4;
    {
        info!(target: "bootstrap", "Create offset mapping");
        let current_page_table_paddr: u64 = unsafe { utils::cr3() }.into();
        // Bootboot does a mem map at 0x0
        current_page_table = unsafe { &mut *(current_page_table_paddr as *mut _) };

        let identity_mapping = |addr: PAddr| {
            let value: u64 = addr.into();
            VAddr::new(value)
        };

        let identity_translate = |l4: &PML4, addr: VAddr| unsafe {
            let l3_entry = l4[pml4_index(addr)];
            let l3_paddr = l3_entry.get_address();
            let l3_vaddr = identity_mapping(l3_paddr);
            let l3: &PDPT = l3_vaddr.as_mut_ptr();
            let l2_entry = l3[pdpt_index(addr)];
            let l2_paddr = l2_entry.get_address();
            let l2_vaddr = identity_mapping(l2_paddr);
            let l2: &PD = l2_vaddr.as_mut_ptr();
            let l1_entry = l2[pd_index(addr)];
            let l1_paddr = l1_entry.get_address();
            let l1_vaddr = identity_mapping(l1_paddr);
            let l1: &PT = l1_vaddr.as_mut_ptr();
            let l0_entry = l1[pt_index(addr)];
            let l0_paddr = l0_entry.get_address();
            l0_paddr
        };

        // Create the page table entries
        // The location where all of memory is mapped to.
        // 0xFFFF_FF00_0000_0000 (entry 510 in P4)
        for i in 0..512usize {
            let pdpt_flags =
                PDPTEntry::PDPT_P | PDPTEntry::PDPT_RW | PDPTEntry::PDPT_G | PDPTEntry::PDPT_PS;
            let paddr = PAddr::new(i as u64 * 1024 * 1024 * 1024);
            let pdpt_entry = PDPTEntry::new(paddr, pdpt_flags);

            unsafe { MEM_MAP_STACK.0[i] = pdpt_entry };
        }

        let target_vaddr = unsafe { &MEM_MAP_STACK.0 as *const [PDPTEntry] as *const u8 as u64 };
        let target_paddr_in_global =
            identity_translate(current_page_table, VAddr::new(target_vaddr));
        let pml4_flags = PML4Entry::PML4_P | PML4Entry::PML4_RW;
        let new_pml4_entry = PML4Entry::new(target_paddr_in_global, pml4_flags);
        current_page_table[510] = new_pml4_entry;

        utils::flush_all();
        info!(target: "bootstrap", "Offset mapping complete");
    }

    let mut free_regions: Vec<MemoryRegion, heapless::consts::U32> = Vec::new();
    {
        let mem_map_entries = unsafe { crate::bootboot::bootboot.get_mmap_entries() };
        for entry in mem_map_entries {
            if !entry.is_free() {
                continue;
            }

            let entry_start = entry.ptr();
            let entry_end = entry.end_address() as usize;

            let size = entry_end - entry_start;
            free_regions
                .push(MemoryRegion::new(entry_start.into(), size))
                .unwrap();
        }
    }

    {
        info!(target: "bootstrap", "Create kernel stacks");

        let addr_mapping = |addr: PAddr| {
            let value: u64 = addr.into();
            VAddr::new(value + globals::MEM_MAP_OFFSET_LOCATION)
        };

        let p4_index = pml4_index(VAddr::new(globals::KERNEL_STACK_START as u64));
        let p3_index = pdpt_index(VAddr::new(globals::KERNEL_STACK_START as u64));

        let l3_entry = current_page_table[p4_index];
        let l3_paddr = l3_entry.get_address();
        let l3_vaddr = addr_mapping(l3_paddr);
        let l3: &mut PDPT = unsafe { l3_vaddr.as_mut_ptr() };

        let l2_entry = l3[p3_index];
        let is_present = l2_entry.is_present();
        assert!(!is_present);

        // Allocate new kernel stacks.
        let allocate_stack = |free_regions: &mut Vec<MemoryRegion, heapless::consts::U32>| {
            for region in free_regions {
                let allocated = region.try_allocate(
                    globals::KERNEL_STACK_NUM_PAGES * 1024 * 1024 * 2, // 2MiB * num pagse
                    2 * 1024 * 1024,                                   // 2MiB
                );

                if let Some(addr) = allocated {
                    return addr;
                }
            }

            panic!("Not enough memory");
        };
        // TODO: we only map upto 1GiB
        let num_cores = unsafe { crate::bootboot::bootboot.numcores };
        for i in 0..(num_cores as usize) {
            // const SIZE_OF_STACK: usize = (globals::KERNEL_STACK_NUM_PAGES + 1) * 2 * 1024 * 1024;
            // let stack_start = globals::KERNEL_STACK_START + SIZE_OF_STACK * i;
            let allocate_addr = allocate_stack(&mut free_regions);
            for page in 0..globals::KERNEL_STACK_NUM_PAGES {
                let pd_index = (globals::KERNEL_STACK_NUM_PAGES + 1) * i + page;
                let pd_flags = PDEntry::PD_P | PDEntry::PD_G | PDEntry::PD_PS | PDEntry::PD_RW;
                let pd_entry = PDEntry::new(allocate_addr + (page * 2 * 1024 * 1024), pd_flags);
                unsafe { KERNEL_STACK_PD_ENTRIES.0[pd_index] = pd_entry };
            }
        }

        // Store the created table.
        let translate = |l4: &PML4, addr: VAddr| unsafe {
            let l3_entry = l4[pml4_index(addr)];
            let l3_paddr = l3_entry.get_address();
            let l3_vaddr = addr_mapping(l3_paddr);
            let l3: &PDPT = l3_vaddr.as_mut_ptr();
            let l2_entry = l3[pdpt_index(addr)];
            let l2_paddr = l2_entry.get_address();
            let l2_vaddr = addr_mapping(l2_paddr);
            let l2: &PD = l2_vaddr.as_mut_ptr();
            let l1_entry = l2[pd_index(addr)];
            let l1_paddr = l1_entry.get_address();
            let l1_vaddr = addr_mapping(l1_paddr);
            let l1: &PT = l1_vaddr.as_mut_ptr();
            let l0_entry = l1[pt_index(addr)];
            let l0_paddr = l0_entry.get_address();
            l0_paddr
        };
        let pd_entries_vaddr =
            unsafe { &KERNEL_STACK_PD_ENTRIES.0 as *const [PDEntry] as *const u8 as u64 };
        let pd_entries_paddr = translate(current_page_table, pd_entries_vaddr.into());
        let pdpt_flags = PDPTEntry::PDPT_P | PDPTEntry::PDPT_G | PDPTEntry::PDPT_RW;
        let pdpt_entry = PDPTEntry::new(pd_entries_paddr, pdpt_flags);
        l3[p3_index] = pdpt_entry;

        info!(target: "bootstrap", "Create kernel stacks complete");
    }

    {
        info!(target: "bootstrap", "Initialize TLS");
        super::tls::initialize_tls(&mut free_regions);
        info!(target: "bootstrap", "Initialize TLS complete");
    }

    {
        info!(target: "bootstrap", "Initialize GDT");
        super::gdt::initialize_gdt();
        info!(target: "bootstrap", "GDT ready");
    }

    {
        info!(target: "bootstrap", "Initialize IDT");
        super::interrupts::initialize_idt();
        info!(target: "bootstrap", "IDT ready");
    }

    {
        info!(target: "bootstrap", "load interrupts");
        super::interrupts::load_interrupts_bsp().unwrap();
        info!(target: "bootstrap", "loaded interrupts");
    }

    {
        info!(target: "bootstrap", "Kernel stack switching");

        let start = globals::KERNEL_STACK_START;
        let proc_count = 0;

        let stack_start =
            start + ((globals::KERNEL_STACK_NUM_PAGES + 1) * 2 * 1024 * 1024 * proc_count);
        let stack_end = stack_start + (globals::KERNEL_STACK_NUM_PAGES * 2 * 1024 * 1024);
        let aligned_stack_end = align::align_down(stack_end, globals::STACK_ALIGN);

        info!(target: "bootstrap", "Kernel stack switching to {:x}", aligned_stack_end);
        // Switch to level 2.
        unsafe {
            asm!("
                mov rsp, {0}
                mov rbp, {0}
                jmp {1}
                ", in(reg) aligned_stack_end, sym initialize_bootstrap_core3, options(noreturn));
        }
    }
}

fn initialize_bootstrap_core3() -> ! {
    info!(target: "bootstrap", "CPU Core ready. Is BSP: true, Core ID: {}", super::cpu_locals::PROCESSOR_ID.get());
    crate::main_bsp()
}
