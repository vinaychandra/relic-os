use crossbeam_utils::atomic::AtomicCell;
use relic_abi::{cap::CapabilityErrors, syscall::SystemCall};
use x86_64::{
    registers::model_specific::{FsBase, KernelGsBase, LStar},
    VirtAddr,
};

use crate::capability::TaskStatus;

#[derive(Default, Debug, Getters, Setters)]
#[getset(get = "pub", set = "pub")]
#[repr(C)]
pub struct Registers {
    // Scratch registers start
    // Parameter registers
    rdi: u64,
    rsi: u64,
    rdx: u64, // Return 2
    rcx: u64,
    r8: u64,
    r9: u64,

    rax: u64, // Return 1

    r10: u64,
    r11: u64,
    // Scratch registers end

    // Preserved registers
    rbx: u64,
    r12: u64,
    r13: u64,
    r14: u64,
    r15: u64,
    rbp: u64,

    rsp: u64,
    rip: u64,
    rflags: u64,

    /// TCB location.
    fs: u64,
}

impl Registers {
    pub const fn empty() -> Self {
        Self {
            rdi: 0,
            rsi: 0,
            rdx: 0,
            rcx: 0,
            r8: 0,
            r9: 0,
            rax: 0,
            r10: 0,
            r11: 0,
            rbx: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
            rbp: 0,
            rsp: 0,
            rip: 0,
            rflags: 0,
            fs: 0,
        }
    }
    pub fn switch_to(&mut self, syscall_data: Option<(CapabilityErrors, u64, u64)>) -> TaskStatus {
        user_switching_fn(self, syscall_data)
    }
}

#[thread_local]
static mut THREAD_SWITCH_RSP_RBP: (u64, u64) = (0, 0);

/// This function sets the CPU Register so that the syscall will call into [`syscall_entry_fn`] function.
fn set_syscall_location(syscall_entry: *const ()) {
    LStar::write(x86_64::VirtAddr::new(syscall_entry as u64));
}

/// Switch to the user code. There are two modes for this to run.
/// Enabling `syscall_mode` will use syscall/sysret pair which is
/// faster than `iret` but cannot restore all registers.
fn user_switching_fn(
    registers: &mut Registers,
    syscall: Option<(CapabilityErrors, u64, u64)>,
) -> TaskStatus {
    unsafe {
        // Store the current stack info.
        let rsp: u64;
        let rbp: u64;
        asm!("
            mov {0}, rsp
            mov {1}, rbp
        ", out(reg) rsp, out(reg) rbp);
        THREAD_SWITCH_RSP_RBP = (rsp, rbp);
    }

    // TODO: we only need to set this once.
    set_syscall_location(syscall_entry_fn as *const ());

    if let Some(data) = syscall {
        let cap_error = data.0.to_u64();
        // Load FsBase for user.
        FsBase::write(VirtAddr::new(registers.fs));
        unsafe {
            asm!("
            mov rsp, rdx
            mov rbp, rsi
            sysretq
        ",
        in("rdi") data.1, in("rsi") registers.rbp, in("rax") cap_error,
        in("rbx") registers.rbx, in("rcx") registers.rip, in("rdx") registers.rsp,
        in("r8") data.2, in("r9") registers.r9, in("r10") registers.r10,
        in("r11") registers.rflags, in("r12") registers.r12, in("r13") registers.r13,
        in("r14") registers.r14, in("r15") registers.r15)
        };
    } else {
        todo!()
    }

    unsafe {
        asm!(
            "user_fn_resume_point:
            nop
            ",
            out("rax") _, out("rbx") _, out("rcx") _, out("rdx") _, out("rsi") _,
            out("rdi") _, out("r8") _, out("r9") _, out("r10") _, out("r11") _, out("r12") _,
            out("r13") _, out("r14") _, out("r15") _,
        );
        *registers = REGISTERS.take();
        debug!(target: "user_future", "Thread returned from usermode by making a syscall.");
    }

    NEXT_STATE.take()
}

// Syscall: rcx -> rdi (IP) ... rdi -> info
#[inline(never)]
#[naked]
unsafe extern "C" fn syscall_entry_fn() {
    // naked to retrieve the values and not corrupt stack. We want to read the stack information here.
    asm!("
        mov r10, rsp
        mov rax, rbp
        jmp {0}
    ", sym syscall_entry_fn_2, options(noreturn));
}

/// This is used to store the register state and provide it back to the kernel stack.
#[thread_local]
static REGISTERS: AtomicCell<Registers> = AtomicCell::new(Registers::empty());

#[thread_local]
static NEXT_STATE: AtomicCell<TaskStatus> = AtomicCell::new(TaskStatus::Unknown);

unsafe extern "C" fn syscall_entry_fn_2(
    a: u64,
    b: u64,
    c: u64,
    user_stored_ip: *const (),
    d: u64,
    e: u64,
) {
    // Once we store the stack, we capture the remaining registers so that we can restore them as needed
    // at a later point in time.
    let user_rsp: *const ();
    let user_rbp: *const ();
    let rbx: u64;
    let r12: u64;
    let r13: u64;
    let r14: u64;
    let r15: u64;
    let rflags: u64;
    asm!("nop", 
        out("r10") user_rsp, out("rax") user_rbp,
        out("rbx") rbx, out("r12") r12, out("r13") r13,
        out("r14") r14, out("r15") r15, out("r11") rflags);

    let old_fs = FsBase::read().as_u64();
    FsBase::write(KernelGsBase::read());
    let mut regs = Registers::empty();
    regs.rsp = user_rsp as u64;
    regs.rbp = user_rbp as u64;
    regs.rip = user_stored_ip as u64;
    regs.rbx = rbx;
    regs.r12 = r12;
    regs.r13 = r13;
    regs.r14 = r14;
    regs.r15 = r15;
    regs.rflags = rflags;
    regs.fs = old_fs;
    REGISTERS.store(regs);

    let syscall = SystemCall::from_regs(a, b, c, d, e).ok();
    NEXT_STATE.store(TaskStatus::SyscalledAndWaiting(syscall));

    let (rsp, rbp) = THREAD_SWITCH_RSP_RBP;
    asm!(
        "
        mov rbp, {1}
        mov rsp, {0}
        jmp user_fn_resume_point
    ", in(reg) rsp, in(reg) rbp, options(noreturn));
}
