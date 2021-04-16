use std::{any::Any, marker::PhantomData, ptr::NonNull};

use relic_abi::{cap::CapabilityErrors, SetDefault};
use spin::RwLock;

use crate::{
    addr::PAddrGlobal,
    arch::{
        globals::BASE_PAGE_LENGTH,
        paging::table::{PDEntry, PDPTEntry, PTEntry, PD, PDPT, PT},
    },
    capability::UntypedDescriptor,
    util::managed_arc::{ManagedArc, ManagedArcAny, ManagedWeakPool1Arc},
};

/// Page length used in current kernel. This is `BASE_PAGE_LENGTH` in x86_64.
pub const PAGE_LENGTH: usize = BASE_PAGE_LENGTH;

/// PML4 page table descriptor.
#[derive(Debug)]
pub struct PML4Descriptor {
    pub(super) start_paddr: PAddrGlobal,
    #[allow(dead_code)]
    pub(super) next: Option<ManagedArcAny>,
}

/// PML4 page table capability.
pub type PML4Cap = ManagedArc<RwLock<PML4Descriptor>>;

/// PDPT page table descriptor.
#[derive(Debug)]
pub struct PDPTDescriptor {
    pub(super) mapped_weak_pool: ManagedWeakPool1Arc,
    start_paddr: PAddrGlobal,
    #[allow(dead_code)]
    next: Option<ManagedArcAny>,
}

/// PDPT page table capability.
pub type PDPTCap = ManagedArc<RwLock<PDPTDescriptor>>;

/// PD page table descriptor.
#[derive(Debug)]
pub struct PDDescriptor {
    mapped_weak_pool: ManagedWeakPool1Arc,
    start_paddr: PAddrGlobal,
    #[allow(dead_code)]
    next: Option<ManagedArcAny>,
}

/// PD page table capability.
pub type PDCap = ManagedArc<RwLock<PDDescriptor>>;

/// PT page table descriptor.
#[derive(Debug)]
pub struct PTDescriptor {
    mapped_weak_pool: ManagedWeakPool1Arc,
    start_paddr: PAddrGlobal,
    #[allow(dead_code)]
    next: Option<ManagedArcAny>,
}

/// PT page table capability.
pub type PTCap = ManagedArc<RwLock<PTDescriptor>>;

/// Page descriptor.
#[derive(Debug)]
pub struct PageDescriptor<T: SetDefault + Any> {
    pub(super) mapped_weak_pool: ManagedWeakPool1Arc,
    pub(super) start_paddr: PAddrGlobal,
    #[allow(dead_code)]
    pub(super) next: Option<ManagedArcAny>,
    pub(super) _marker: PhantomData<T>,
}

/// Page capability.
pub type PageCap<T> = ManagedArc<RwLock<PageDescriptor<T>>>;

macro_rules! paging_cap {
    ( $cap:ty, $desc:tt, $paging:ty, $entry:tt, $map_fn:ident, $sub_cap:ty, $access:expr ) => {
        impl $cap {
            pub fn retype_from(untyped: &mut UntypedDescriptor) -> Result<Self, CapabilityErrors> {
                let mut arc: Option<Self> = None;

                let start_paddr = unsafe { untyped.allocate(BASE_PAGE_LENGTH, BASE_PAGE_LENGTH)? };

                let mapped_weak_pool = unsafe {
                    ManagedWeakPool1Arc::create(untyped.allocate(
                        ManagedWeakPool1Arc::inner_type_length(),
                        ManagedWeakPool1Arc::inner_type_alignment(),
                    )?)
                };

                unsafe {
                    untyped.derive(
                        Self::inner_type_length(),
                        Self::inner_type_alignment(),
                        |paddr, next_child| {
                            let mut desc = $desc {
                                mapped_weak_pool: mapped_weak_pool,
                                start_paddr: start_paddr,
                                next: next_child,
                            };

                            for item in desc.write().iter_mut() {
                                *item = $entry::empty();
                            }

                            arc = Some(Self::new(paddr, RwLock::new(desc)));

                            arc.clone().unwrap()
                        },
                    )?;
                }

                Ok(arc.unwrap())
            }

            pub fn $map_fn(
                &mut self,
                index: usize,
                sub: &$sub_cap,
            ) -> Result<(), CapabilityErrors> {
                let mut current_desc = self.write();
                let current = current_desc.write();
                let sub_desc = sub.read();
                if current[index].is_present() {
                    return Err(CapabilityErrors::CapabilityAlreadyOccupied);
                }

                sub_desc
                    .mapped_weak_pool
                    .downgrade_at(self.clone(), 0)
                    .map_err(|_| CapabilityErrors::MemoryAlreadyMapped)?;
                current[index] = $entry::new(sub_desc.start_paddr.to_paddr(), $access);
                Ok(())
            }
        }

        impl $desc {
            pub fn start_paddr(&self) -> PAddrGlobal {
                self.start_paddr
            }

            pub fn length(&self) -> usize {
                BASE_PAGE_LENGTH
            }

            fn page_object(&self) -> NonNull<$paging> {
                let addr: u64 = self.start_paddr.into();
                NonNull::new(addr as _).unwrap()
            }

            pub fn read(&self) -> &$paging {
                unsafe { self.page_object().as_ref() }
            }

            fn write(&mut self) -> &mut $paging {
                unsafe { self.page_object().as_mut() }
            }
        }
    };
}

paging_cap!(
    PDPTCap,
    PDPTDescriptor,
    PDPT,
    PDPTEntry,
    map_pd,
    PDCap,
    PDPTEntry::PDPT_P | PDPTEntry::PDPT_RW | PDPTEntry::PDPT_US
);
paging_cap!(
    PDCap,
    PDDescriptor,
    PD,
    PDEntry,
    map_pt,
    PTCap,
    PDEntry::PD_P | PDEntry::PD_RW | PDEntry::PD_US
);

impl PTCap {
    /// Create a Page table in the given untyped memory.
    pub fn retype_from(untyped: &mut UntypedDescriptor) -> Result<Self, CapabilityErrors> {
        let mut arc: Option<Self> = None;

        let start_paddr = unsafe { untyped.allocate(BASE_PAGE_LENGTH, BASE_PAGE_LENGTH)? };

        let mapped_weak_pool = unsafe {
            ManagedWeakPool1Arc::create(untyped.allocate(
                ManagedWeakPool1Arc::inner_type_length(),
                ManagedWeakPool1Arc::inner_type_alignment(),
            )?)
        };

        unsafe {
            untyped.derive(
                Self::inner_type_length(),
                Self::inner_type_alignment(),
                |paddr, next_child| {
                    let mut desc = PTDescriptor {
                        mapped_weak_pool,
                        start_paddr,
                        next: next_child,
                    };

                    for item in desc.write().iter_mut() {
                        *item = PTEntry::empty();
                    }

                    arc = Some(Self::new(paddr, RwLock::new(desc)));

                    arc.clone().unwrap()
                },
            )?;
        }

        Ok(arc.unwrap())
    }

    /// Map a page in this PT.
    pub fn map_page<T: SetDefault + Any + core::fmt::Debug>(
        &mut self,
        index: usize,
        sub: &PageCap<T>,
        perms: crate::capability::MapPermissions,
    ) -> Result<(), CapabilityErrors> {
        let mut current_desc = self.write();
        let current = current_desc.write();
        let sub_desc = sub.read();
        if current[index].is_present() {
            return Err(CapabilityErrors::CapabilityAlreadyOccupied);
        }

        let mut flags = PTEntry::PT_P | PTEntry::PT_US;
        if perms.contains(crate::capability::MapPermissions::WRITE) {
            flags |= PTEntry::PT_RW;
        }
        if !perms.contains(crate::capability::MapPermissions::EXECUTE) {
            flags |= PTEntry::PT_XD;
        }

        sub_desc
            .mapped_weak_pool
            .downgrade_at(self.clone(), 0)
            .map_err(|_| CapabilityErrors::MemoryAlreadyMapped)?;
        current[index] = PTEntry::new(sub_desc.start_paddr.to_paddr(), flags);

        Ok(())
    }
}

impl PTDescriptor {
    pub fn start_paddr(&self) -> PAddrGlobal {
        self.start_paddr
    }

    pub fn length(&self) -> usize {
        BASE_PAGE_LENGTH
    }

    fn page_object(&self) -> NonNull<PT> {
        let addr: u64 = self.start_paddr.into();
        NonNull::new(addr as _).unwrap()
    }

    pub fn read(&self) -> &PT {
        unsafe { self.page_object().as_ref() }
    }

    fn write(&mut self) -> &mut PT {
        unsafe { self.page_object().as_mut() }
    }
}
