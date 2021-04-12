use crate::addr::{PAddr, VAddr};

/// Contains page-table root pointer.
#[inline]
unsafe fn cr3() -> PAddr {
    let ret: u64;
    asm!("mov {0}, cr3", out(reg) ret, options(readonly, preserves_flags));
    ret.into()
}

/// Switch page-table PML4 pointer.
#[inline]
unsafe fn cr3_write(val: PAddr) {
    asm!("mov cr3, {0}", in(reg) val.into(): u64)
}

/// Invalidate the given address in the TLB using the `invlpg` instruction.
///
/// # Safety
///
/// This function is unsafe as it causes a general protection fault (GP) if the current privilege
/// level is not 0.
#[inline]
pub unsafe fn flush(vaddr: VAddr) {
    asm!("invlpg [{0}]", in(reg) vaddr.into(): u64, options(nostack))
}

/// Invalidate the TLB completely by reloading the CR3 register.
///
/// # Safety
///
/// This function is unsafe as it causes a general protection fault (GP) if the current privilege
/// level is not 0.
#[inline]
pub unsafe fn flush_all() {
    cr3_write(cr3())
}

/// Switch to a PML4 page table.
///
/// # Safety
/// `paddr` must point to a valid PML4 page table.
#[inline]
pub unsafe fn switch_to(paddr: PAddr) {
    cr3_write(paddr.into());
}
