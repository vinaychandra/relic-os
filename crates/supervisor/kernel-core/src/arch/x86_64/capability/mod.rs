use std::any::TypeId;

use crate::{
    addr::PAddr,
    arch::capability::paging::{PDCap, PDPTCap, PML4Cap, PTCap},
    util::managed_arc::{ManagedArc, ManagedArcAny},
};

/// Convert the ['ManagedArcAny`] to a strong type (arch-specific) and call
/// the provided function. The first parameter to the function
/// is the strong typed object and other parameters are passed as-is.
macro_rules! doto_arch_any {
    ($any:expr, $f:tt $(,$param:expr)*) => {
        if $any.is::<crate::arch::capability::paging::PML4Cap>() {
            $f ($any.into(): crate::arch::capability::paging::PML4Cap, $($param),*)
        } else if $any.is::<crate::arch::capability::paging::PDPTCap>() {
            $f ($any.into(): crate::arch::capability::paging::PDPTCap, $($param),*)
        } else if $any.is::<crate::arch::capability::paging::PDCap>() {
            $f ($any.into(): crate::arch::capability::paging::PDCap, $($param),*)
        } else if $any.is::<crate::arch::capability::paging::PTCap>() {
            $f ($any.into(): crate::arch::capability::paging::PTCap, $($param),*)
        } else {
            panic!("Cannot match the type of any")
        }
    };
}

pub mod paging;

/// Create a managed Arc (capability) from an address of an
/// architecture-specific kernel object. The `type_id` should be a
/// [`TypeId`] of an architecture-specific capability. If the `type_id` is not
/// recognized, `None` is returned.
///
/// # Safety
/// `ptr` must be a physical address pointing to a valid kernel object
/// of type `type_id`.
pub unsafe fn upgrade_arch_any(ptr: PAddr, type_id: TypeId) -> Option<ManagedArcAny> {
    if type_id == TypeId::of::<PML4Cap>() {
        Some({ ManagedArc::<PML4Cap>::from_ptr(ptr).ok()? }.into())
    } else if type_id == TypeId::of::<PDPTCap>() {
        Some({ ManagedArc::<PDPTCap>::from_ptr(ptr).ok()? }.into())
    } else if type_id == TypeId::of::<PDCap>() {
        Some({ ManagedArc::<PDCap>::from_ptr(ptr).ok()? }.into())
    } else if type_id == TypeId::of::<PTCap>() {
        Some({ ManagedArc::<PTCap>::from_ptr(ptr).ok()? }.into())
    } else {
        None
    }
}
