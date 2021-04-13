/// Convert the ['ManagedArcAny`] to a strong type and call
/// the provided function. The first parameter to the function
/// is the strong typed object and other parameters are passed as-is.
macro_rules! doto_any {
    ($any:expr, $f:tt $(,$param:expr)*) => {
        if $any.is::<crate::capability::CPoolCap>() {
            $f ($any.into(): crate::capability::CPoolCap, $($param),*)
        } else if $any.is::<crate::capability::UntypedCap>() {
            $f ($any.into(): crate::capability::UntypedCap, $($param),*)
        // } else if $any.is::<crate::capability::TaskCap>() {
        //     $f ($any.into(): crate::capability::TaskCap, $($param),*)
        // } else if $any.is::<crate::capability::RawPageCap>() {
        //     $f ($any.into(): crate::capability::RawPageCap, $($param),*)
        // } else if $any.is::<crate::capability::TaskBufferPageCap>() {
        //     $f ($any.into(): crate::capability::TaskBufferPageCap, $($param),*)
        // } else if $any.is::<crate::capability::ChannelCap>() {
        //     $f ($any.into(): crate::capability::ChannelCap, $($param),*)
        } else {
            doto_arch_any!($any, $f $(,$param)*)
        }
    }
}

/// Capability pool capability implementation.
mod cpool;
/// Untyped capability implementation.
mod untyped;

use std::any::TypeId;

pub use cpool::*;
pub use untyped::*;

use crate::{
    addr::PAddr,
    util::managed_arc::{ManagedArc, ManagedArcAny},
};

/// Create a managed Arc (capability) from an address of an kernel
/// object (architecture-specific or general). The `type_id` should be
/// a [TypeId](https://doc.rust-lang.org/std/any/struct.TypeId.html)
/// of a capability. If the `type_id` is not recognized, `None` is
/// returned.
///
/// # Safety
///
/// `ptr` must be a physical address pointing to a valid kernel object
/// of type `type_id`.
pub unsafe fn upgrade_any(ptr: PAddr, type_id: TypeId) -> Option<ManagedArcAny> {
    if type_id == TypeId::of::<CPoolCap>() {
        Some({ ManagedArc::<CPoolCap>::from_ptr(ptr).ok()? }.into())
    } else if type_id == TypeId::of::<UntypedCap>() {
        Some({ ManagedArc::<UntypedCap>::from_ptr(ptr).ok()? }.into())
    // } else if type_id == TypeId::of::<TaskCap>() {
    //     Some({ ManagedArc::<TaskCap>::from_ptr(ptr) }.into())
    // } else if type_id == TypeId::of::<RawPageCap>() {
    //     Some({ ManagedArc::<RawPageCap>::from_ptr(ptr) }.into())
    // } else if type_id == TypeId::of::<TaskBufferPageCap>() {
    //     Some({ ManagedArc::<TaskBufferPageCap>::from_ptr(ptr) }.into())
    // } else if type_id == TypeId::of::<ChannelCap>() {
    //     Some({ ManagedArc::<ChannelCap>::from_ptr(ptr) }.into())
    } else {
        crate::arch::capability::upgrade_arch_any(ptr, type_id)
    }
}

/// Drop an architecture-specific `any` capability. `ManagedArcAny` is
/// not itself droppable. It must be converted to its real type before
/// dropping.
pub fn drop_any(any: ManagedArcAny) {
    doto_any!(any, drop)
}
