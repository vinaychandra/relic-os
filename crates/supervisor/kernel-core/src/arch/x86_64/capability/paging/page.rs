use relic_abi::{cap::CapabilityErrors, SetDefault};
use spin::RwLock;
use std::{any::Any, marker::PhantomData, mem};

use crate::{
    addr::PAddr,
    arch::{
        capability::paging::page_cap::{PageCap, PageDescriptor},
        paging::BASE_PAGE_LENGTH,
    },
    capability::UntypedDescriptor,
    prelude::MemoryObject,
    util::{
        managed_arc::ManagedWeakPool1Arc,
        memory_object::{UniqueReadGuard, UniqueWriteGuard},
    },
};

/// Page length used in current kernel. This is `BASE_PAGE_LENGTH` in x86_64.
pub const PAGE_LENGTH: usize = BASE_PAGE_LENGTH;

impl<T: SetDefault + Any> PageCap<T> {
    pub fn retype_from(untyped: &mut UntypedDescriptor) -> Result<Self, CapabilityErrors> {
        unsafe {
            Self::bootstrap(
                untyped.allocate(BASE_PAGE_LENGTH, BASE_PAGE_LENGTH)?,
                untyped,
            )
        }
    }

    pub unsafe fn bootstrap(
        start_paddr: PAddr,
        untyped: &mut UntypedDescriptor,
    ) -> Result<Self, CapabilityErrors> {
        assert!(
            mem::size_of::<T>() <= PAGE_LENGTH,
            "A page cap must fit in a page"
        );

        let mut arc: Option<Self> = None;

        let mapped_weak_pool = ManagedWeakPool1Arc::create(untyped.allocate(
            ManagedWeakPool1Arc::inner_type_length(),
            ManagedWeakPool1Arc::inner_type_alignment(),
        )?);

        untyped.derive(
            Self::inner_type_length(),
            Self::inner_type_alignment(),
            |paddr, next_child| {
                let mut desc = PageDescriptor::<T> {
                    mapped_weak_pool,
                    start_paddr,
                    next: next_child,
                    _marker: PhantomData,
                };

                desc.write().set_default();

                arc = Some(Self::new(paddr, RwLock::new(desc)));

                arc.clone().unwrap().into()
            },
        )?;

        Ok(arc.unwrap())
    }

    pub const fn length() -> usize {
        BASE_PAGE_LENGTH
    }
}

impl<T: SetDefault + Any> PageDescriptor<T> {
    pub fn start_paddr(&self) -> PAddr {
        self.start_paddr
    }

    pub fn length(&self) -> usize {
        BASE_PAGE_LENGTH
    }

    fn page_object(&self) -> MemoryObject<T> {
        unsafe { MemoryObject::new(self.start_paddr()) }
    }

    pub fn read(&self) -> UniqueReadGuard<T> {
        unsafe { UniqueReadGuard::new(self.page_object()) }
    }

    pub fn write(&mut self) -> UniqueWriteGuard<T> {
        unsafe { UniqueWriteGuard::new(self.page_object()) }
    }
}
