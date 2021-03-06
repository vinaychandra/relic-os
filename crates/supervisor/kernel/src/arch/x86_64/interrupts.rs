use ::acpi::{AcpiTables, InterruptModel};
use x86_64::{
    instructions::port::Port,
    structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode},
};

use crate::arch::{gdt, globals, interrupts::acpi::MemoryHandler};

pub mod acpi;
pub mod apic;

/// Index of interrupts. This is the index where IRQs are raised
/// on PIC.
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = 32,
    Keyboard,
    Spurious,
    Error,
    HpetTimer, // 36
}

impl InterruptIndex {
    /// Get the index in IRQ list for the given interrupt.
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// Convert the index to usize.
    pub fn as_usize(self) -> usize {
        usize::from(self.as_u8())
    }
}

static mut IDT: InterruptDescriptorTable = InterruptDescriptorTable::new();

pub fn initialize_idt() {
    unsafe {
        IDT.double_fault
            .set_handler_fn(double_fault_handler)
            .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX as u16);
        IDT.page_fault.set_handler_fn(page_fault_handler);

        IDT.general_protection_fault.set_handler_fn(unhandled_fault);
        IDT.invalid_opcode.set_handler_fn(unhandled_fault_noerr);

        IDT.load();
    }
}

/// Handler than be used for non-standard faults.
extern "x86-interrupt" fn unhandled_fault(stack_frame: InterruptStackFrame, error_code: u64) {
    error!(
        target: "unhandled_fault",
        "EXCEPTION: Unhandled FAULT\n{:#?}\nError Code: {}",
        stack_frame,
        error_code
    );

    loop {
        x86_64::instructions::hlt();
    }
}

/// Handler than be used for non-standard faults.
extern "x86-interrupt" fn unhandled_fault_noerr(stack_frame: InterruptStackFrame) {
    error!(target: "unhandled_fault_noerr", "EXCEPTION: Unhandled FAULT\n{:#?}", stack_frame);
    loop {
        x86_64::instructions::hlt();
    }
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) -> ! {
    error!(
        target: "DoubleFaultHandler",
        "EXCEPTION: DOUBLE FAULT\n{:#?}\nError Code: {}",
        stack_frame,
        error_code
    );
    loop {
        x86_64::instructions::hlt();
    }
}

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2;
    error!(
        target: "PageFaultHandler",
        "EXCEPTION: PAGE FAULT\r\n{:#?}\r\nError Code: {:?}\r\nAccessed Address: {:?}",
        stack_frame,
        error_code,
        Cr2::read()
    );
    loop {
        x86_64::instructions::hlt();
    }
}

/// Loads the interrupt mappings and returns the number of AP cores.
pub fn load_interrupts_bsp() -> Result<(), &'static str> {
    info!(target:"interrupts", "Setting up interrupts");

    info!(target:"interrupts", "Disabling PIC");
    disable_pic();
    info!(target:"interrupts", "PIC disabled");

    info!(target:"interrupts", "Load ACPI tables");
    let acpi_addr = unsafe { crate::bootboot::bootboot.arch.x86_64.acpi_ptr as *const u8 as usize };
    info!(target:"interrupts", "ACPI Addr is {:x}", acpi_addr);

    let offset = globals::MEM_MAP_OFFSET_LOCATION;
    let handler = MemoryHandler::new(offset as usize);
    let acpi_tables = unsafe {
        AcpiTables::from_rsdt(handler, 2, acpi_addr).or(Err("ACPI Tables cannot be parsed"))?
    };
    info!(target:"interrupts", "ACPI tables loaded successfully.");

    let platform_info = acpi_tables
        .platform_info()
        .or(Err("Cannot load ACPI platform_info"))?;
    if let InterruptModel::Apic(apic) = platform_info.interrupt_model {
        info!(target:"interrupts", "Enable local APIC");
        self::apic::initialize_lapic();
        info!(target:"interrupts", "Local APIC ready");

        info!(target:"interrupts", "Enable IO APIC");
        self::apic::initialize_ioapic(apic);
        info!(target:"interrupts", "IO APIC ready");
    } else {
        return Err("APIC data not found in ACPI tables.");
    }

    Ok(())
}

fn disable_pic() {
    let mut port1: Port<u8> = Port::new(0xa1);
    let mut port2: Port<u8> = Port::new(0x21);
    unsafe {
        port1.write(0xff);
        port2.write(0xff);
    }
}
