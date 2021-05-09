use core::convert::TryInto;

use relic_abi::{cap::CapabilityErrors, syscall::SystemCall};

/// Make a raw syscall.
/// This first converts the syscall into its register representation and then sends the info
/// to the kernel.
#[inline]
pub fn make_syscall(syscall: &SystemCall) -> Result<(u64, u64), CapabilityErrors> {
    let regs = syscall.as_regs().map_err(|_| CapabilityErrors::Unknown)?;
    let error: u64;
    let a: u64;
    let b: u64;

    unsafe {
        #[cfg(target_feature = "sse")]
        {
            asm!(
                "
                push rbx
                syscall
                pop rbx",
                inout("rdi") regs.0 => a,
                in("rsi") regs.1,
                in("rdx") regs.2,
                inout("r8") regs.3 => b,
                in("r9") regs.4,
                // All caller-saved registers must be marked as clobberred
                out("rax") error,
                out("r10") _, out("r11") _,
                out("xmm0") _, out("xmm1") _, out("xmm2") _, out("xmm3") _,
                out("xmm4") _, out("xmm5") _, out("xmm6") _, out("xmm7") _,
                out("xmm8") _, out("xmm9") _, out("xmm10") _, out("xmm11") _,
                out("xmm12") _, out("xmm13") _, out("xmm14") _, out("xmm15") _,
            );
        }

        #[cfg(not(target_feature = "sse"))]
        {
            asm!(
                "push rbx
                syscall
                pop rbx",
                inout("rdi") regs.0 => a,
                in("rsi") regs.1,
                in("rdx") regs.2,
                inout("r8") regs.3 => b,
                in("r9") regs.4,
                // All caller-saved registers must be marked as clobberred
                out("rax") error,
                out("r10") _, out("r11") _,
            );
        }
    }

    // Try to convert the kernel returned error code into a capability error.
    let cap: Result<CapabilityErrors, ()> = error.try_into();
    if cap.is_err() {
        // Unable to convert the code. Return Err with unknown.
        return Err(CapabilityErrors::Unknown);
    }

    // In case of success, kernel returns a None error code.
    let value = cap.unwrap();
    if value == CapabilityErrors::None {
        return Ok((a, b));
    } else {
        return Err(value);
    }
}
