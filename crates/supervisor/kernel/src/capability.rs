/*!
Kernel capability support

This is the implementation of the central idea of capability in the kernel.
There are two types of kernel objects at play here. One are capability objects and
other are kernel object data.

The total size of a capability object is fixed at 64bytes to fix in most cache lines.
The capability object tracks most of the properties required by the capabilities and
only spills on to the 'kernel object' when the 64byte space is not enough. For example,
most threads require storage of registers which occupy more than 64bytes. So, these are
pushed onto the remaining memory.

The capability objects also cover the derivation trees. These include memory derivation
trees which track derivations from untyped memory, page derivation trees which track
derivations of virtual memory.

Capabilities in relic do not track the number of owners but use a non-counting Rc:
[`UnsafeRef`].
*/

use relic_abi::cap::CapabilityErrors;
use std::{
    cell::{Ref, RefCell, RefMut},
    ptr::NonNull,
};

use crate::{addr::PAddrGlobal, arch::capability::paging::*, util::unsafe_ref::UnsafeRef};

mod cpool;
pub mod task;
mod untyped;

pub use cpool::*;
pub use task::*;
pub use untyped::*;

/**
Capability data in relic uses enum based dispatch rather than relying on
dynamic memory for performance. Different types of possible capabilties are
stored in this structure.
*/
#[derive(Debug)]
pub enum CapabilityEnum {
    /// Untyped memory capability. See [`UntypedMemory`].
    UntypedMemory(UntypedMemory),
    /// Cpool memory capability. Acts as a storage for capability
    /// objects. See [`Cpool`].
    Cpool(Cpool),
    /// Capability that denotes no capability. Used for marking
    /// empty capability locations.
    EmptyCap,

    /// Level4 paging capability. Also denotes an address space. See [`L4`]
    L4(L4),
    /// Level3 paging capability. See [`L3`]
    L3(L3),
    /// Level2 paging capability. See [`L2`]
    L2(L2),
    /// Level1 paging capability. See [`L1`]
    L1(L1),

    /// Raw page capability. Used for mapping data into address spaces.
    /// Smallest page size: 0x1000 bytes.
    /// See [`RawPageActual`]
    BasePage(BasePage),
    /// Raw page capability. Used for mapping data into address spaces.
    /// Large page size: 0x20_0000 bytes.
    /// See [`RawPageActual`]
    LargePage(LargePage),
    /// Raw page capability. Used for mapping data into address spaces.
    /// Huge page size: 0x4000_0000 bytes.
    /// See [`RawPageActual`]
    HugePage(HugePage),

    /// Kernel thread support. See [`Task`].
    Task(Task),
}

/// Smallest page size: 0x1000 bytes.
pub type BasePage = RawPageActual<0x1000>;
/// Large page size: 0x20_0000 bytes.
pub type LargePage = RawPageActual<0x20_0000>;
/// Huge page size: 0x4000_0000 bytes.
pub type HugePage = RawPageActual<0x4000_0000>;

/// Kernel capability object. This object is a wrapper around
/// [`CapabilityEnum`] with added support for memory derivation tree.
#[derive(Debug)]
pub struct Capability {
    /// The underlying capability information.
    pub capability_data: CapabilityEnum,

    /// Next memory item in the memory derivation tree.
    /// Stores a sibling in memory tree.
    pub next_mem_item: Option<StoredCap>,
    /// Previous memory item in the memory derivation tree.
    /// Stores a sibling in memory tree.
    pub prev_mem_item: Option<StoredCap>,
}

// Compile time 64 byte sized assertion for capability.
assert_eq_size!([u8; 64], RefCell<Capability>);

/// The referenec for a kernel capability. The actual kernel capability objects
/// are [`RefCell<Capability>`] and this type stores a non-counted reference
/// to that capability object.
pub type StoredCap = UnsafeRef<RefCell<Capability>>;

impl Default for Capability {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl Capability {
    /// Traverse to the next item in task linked list.
    pub fn get_next_task_item_mut(&mut self) -> &mut Option<StoredCap> {
        match &mut self.capability_data {
            CapabilityEnum::Task(l) => &mut l.next_task_item,
            _ => panic!("Unsupported"),
        }
    }

    /// Traverse to the previous item in task linked list.
    pub fn get_prev_task_item_mut(&mut self) -> &mut Option<StoredCap> {
        match &mut self.capability_data {
            CapabilityEnum::Task(l) => &mut l.prev_task_item,
            _ => panic!("Unsupported"),
        }
    }

    /// Traverse to the next item in paging tree.
    pub fn get_next_paging_item_mut(&mut self) -> &mut Option<StoredCap> {
        match &mut self.capability_data {
            CapabilityEnum::L3(l) => &mut l.next_paging_item,
            CapabilityEnum::L2(l) => &mut l.next_paging_item,
            CapabilityEnum::L1(l) => &mut l.next_paging_item,
            CapabilityEnum::BasePage(l) => &mut l.next_paging_item,
            CapabilityEnum::LargePage(l) => &mut l.next_paging_item,
            CapabilityEnum::HugePage(l) => &mut l.next_paging_item,
            _ => panic!("Unsupported"),
        }
    }

    /// Traverse to the next item in paging tree.
    pub fn get_prev_paging_item_mut(&mut self) -> &mut Option<StoredCap> {
        match &mut self.capability_data {
            CapabilityEnum::L3(l) => &mut l.prev_paging_item,
            CapabilityEnum::L2(l) => &mut l.prev_paging_item,
            CapabilityEnum::L1(l) => &mut l.prev_paging_item,
            CapabilityEnum::BasePage(l) => &mut l.prev_paging_item,
            CapabilityEnum::LargePage(l) => &mut l.prev_paging_item,
            CapabilityEnum::HugePage(l) => &mut l.prev_paging_item,
            _ => panic!("Unsupported"),
        }
    }

    /// Create an empty new capability object.
    #[inline]
    pub const fn new() -> Self {
        Self {
            capability_data: CapabilityEnum::EmptyCap,
            next_mem_item: None,
            prev_mem_item: None,
        }
    }
}

/**
A faster way to read capabilities.
Because the capability objects are present in a refcell and then behind
an enum, it would take multiple steps for us do a borrow on the refcell
followed by a match on the enum everytime data is required.

This accessor provides a wrapper around the [`Ref`] object of the refcell
of capability providing safe and immediate access to capability object.
We just have to create this object once and use it as a [`CapabilityEnum`]
variant directly.
*/
#[derive(Getters)]
pub struct CapAccessor<'a, T> {
    /// The captured [`Ref`] object. Guarantees that the current object
    /// has the same ownership properties as that object.
    _borrow: Ref<'a, Capability>,

    /// Unsafe pointer to the target capability variant.
    data: NonNull<T>,

    /// An indirect reference to the capability object in which this is a
    /// part of. This is used whenever some function want to store the
    /// data represented by the accessor. This provides an access to the
    /// underlying capability object.
    #[getset(get = "pub")]
    cap: StoredCap,
}

/**
A faster way to read and write capabilities.
Because the capability objects are present in a refcell and then behind
an enum, it would take multiple steps for us do a borrow on the refcell
followed by a match on the enum everytime data is required.

This accessor provides a wrapper around the [`RefMut`] object of the refcell
of capability providing safe and immediate access to capability object.
We just have to create this object once and use it as a [`CapabilityEnum`]
variant directly.
*/
#[derive(Getters)]
pub struct CapAccessorMut<'a, T> {
    /// The captured [`Ref`] object. Guarantees that the current object
    /// has the same ownership properties as that object.
    _borrow: RefMut<'a, Capability>,

    /// Unsafe pointer to the target capability variant.
    data: NonNull<T>,

    /// An indirect reference to the capability object in which this is a
    /// part of. This is used whenever some function want to store the
    /// data represented by the accessor. This provides an access to the
    /// underlying capability object.
    #[getset(get = "pub")]
    cap: StoredCap,
}

impl<T> core::ops::Deref for CapAccessor<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // This is safe because this pointer is valid when the borrow is alive.
        unsafe { self.data.as_ref() }
    }
}

impl<T> core::ops::Deref for CapAccessorMut<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // This is safe because this pointer is valid when the borrow is alive.
        unsafe { self.data.as_ref() }
    }
}

impl<T> core::ops::DerefMut for CapAccessorMut<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // This is safe because this pointer is valid when the borrow is alive.
        unsafe { self.data.as_mut() }
    }
}

macro_rules! cap_create {
    ($cap_name: ty) => {
        paste! {
            impl StoredCap {
                /**
                Create a new [`CapAccessor`] with the provided variant. This will return an
                [`CapabilityErrors::CapabilityMismatch`] if the inner capability type is not the same as the requested type.
                This function will panic will the capability is already mutably borrowed.
                */
                pub fn [< as_ $cap_name:snake >](&self) -> Result<CapAccessor<'_, $cap_name>, CapabilityErrors> {
                    let borrow = self.borrow();
                    let data = if let CapabilityEnum::$cap_name(u) = &borrow.capability_data {
                        u as *const $cap_name as *mut $cap_name
                    } else {
                        return Err(CapabilityErrors::CapabilityMismatch);
                    };
                    Ok(CapAccessor {
                        _borrow: borrow,
                        data: NonNull::new(data).unwrap(),
                        cap: self.clone(),
                    })
                }

                /**
                Create a new [`CapAccessorMut`] with the provided variant. This will return an
                [`CapabilityErrors::CapabilityMismatch`] if the inner capability type is not the same as the requested type.
                This function will panic will the capability is already borrowed.
                */
                pub fn [< as_ $cap_name:snake _mut >](
                    &self,
                ) -> Result<CapAccessorMut<'_, $cap_name>, CapabilityErrors> {
                    let mut borrow = self.borrow_mut();
                    let data = if let CapabilityEnum::$cap_name(u) = &mut borrow.capability_data {
                        u as *mut $cap_name
                    } else {
                        return Err(CapabilityErrors::CapabilityMismatch);
                    };
                    Ok(CapAccessorMut {
                        _borrow: borrow,
                        data: NonNull::new(data).unwrap(),
                        cap: self.clone(),
                    })
                }
            }
        }
    };
}

cap_create!(UntypedMemory);
cap_create!(Cpool);
cap_create!(L4);
cap_create!(L3);
cap_create!(L2);
cap_create!(L1);
cap_create!(BasePage);
cap_create!(LargePage);
cap_create!(HugePage);
cap_create!(Task);

bitflags! {
    /// Permissions when mapping paging into virtual memory.
    pub struct MapPermissions : u8 {
        /// Write permissions for the page.
        const WRITE     = 0b0000_0010;
        /// Execute permissions for the page. In supported architectures,
        /// this page will be marked non executable if this flag is absent.
        const EXECUTE   = 0b0000_0100;
    }
}
