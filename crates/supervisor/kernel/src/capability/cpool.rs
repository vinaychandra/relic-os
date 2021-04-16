use std::{any::Any, ops::Deref};

use relic_abi::{cap::CapabilityErrors, prelude::CAddr};
use spin::RwLock;

use crate::{
    capability::UntypedDescriptor,
    util::managed_arc::{ManagedArc, ManagedArcAny, ManagedWeakPool256Arc},
};

/// Capability pool descriptor.
#[derive(Debug)]
pub struct CPoolDescriptor {
    weak_pool: ManagedWeakPool256Arc,
    #[allow(dead_code)]
    next: Option<ManagedArcAny>,
}

/// Capability pool capability. Reference-counted smart pointer to
/// capability pool descriptor. Capability pool itself is a
/// `ManagedWeakPool` with 256 entries.
///
/// Capability pool capability is used to hold multiple capabilities
/// together so as to be addressable in user-space programs.
pub type CPoolCap = ManagedArc<RwLock<CPoolDescriptor>>;

impl CPoolDescriptor {
    /// Create a new pointer to a capability descriptor using the
    /// index. If nothing is in the entry, `None` is returned.
    pub fn upgrade_any(&self, index: usize) -> Option<ManagedArcAny> {
        self.weak_pool.upgrade_any(index)
    }

    /// Like `upgrade_any`, but returns a value with the specified
    /// type.
    pub fn upgrade<T: Any + core::fmt::Debug>(&self, index: usize) -> Option<ManagedArc<T>> {
        self.weak_pool.upgrade(index)
    }

    /// Downgrade a capability into the capability pool (weak pool) at
    /// a specified index.
    pub fn downgrade_at<T: Any + core::fmt::Debug>(
        &self,
        arc: ManagedArc<T>,
        index: usize,
    ) -> Result<(), CapabilityErrors> {
        self.weak_pool
            .downgrade_at(arc, index)
            .map_err(|_| CapabilityErrors::CapabilityAlreadyOccupied)
    }

    /// Downgrade a capability into the capability pool (weak pool) at
    /// a free index.
    pub fn downgrade_free<T: Any + core::fmt::Debug>(
        &self,
        arc: ManagedArc<T>,
    ) -> Result<usize, CapabilityErrors> {
        self.weak_pool
            .downgrade_free(arc)
            .ok_or(CapabilityErrors::CapabilitySlotsFull)
    }

    /// Downgrade a `ManagedArcAny` into the capability pool (weak
    /// pool) at a specified index.
    pub fn downgrade_any_at(
        &self,
        arc: ManagedArcAny,
        index: usize,
    ) -> Result<(), CapabilityErrors> {
        self.weak_pool
            .downgrade_at(arc, index)
            .map_err(|_| CapabilityErrors::CapabilityAlreadyOccupied)
    }

    /// Downgrade a `ManagedArcAny` into the capability pool (weak
    /// pool) at a free index.
    pub fn downgrade_any_free(&self, arc: ManagedArcAny) -> Result<usize, CapabilityErrors> {
        self.weak_pool
            .downgrade_free(arc)
            .ok_or(CapabilityErrors::CapabilityAlreadyOccupied)
    }

    /// Size of the capability pool.
    pub fn size(&self) -> usize {
        256
    }

    /// Number of capabilities stored currently in the pool. (Conveservative)
    pub fn capability_count(&self) -> usize {
        self.weak_pool.capability_count()
    }
}

impl CPoolCap {
    /// Create a capability pool capability from an untyped
    /// capability.
    pub fn retype_from(untyped: &mut UntypedDescriptor) -> Result<Self, CapabilityErrors> {
        let mut arc: Option<Self> = None;

        let weak_pool = unsafe {
            ManagedWeakPool256Arc::create(untyped.allocate(
                ManagedWeakPool256Arc::inner_type_length(),
                ManagedWeakPool256Arc::inner_type_alignment(),
            )?)
        };

        unsafe {
            untyped.derive(
                Self::inner_type_length(),
                Self::inner_type_alignment(),
                |paddr, next_child| {
                    arc = Some(Self::new(
                        paddr,
                        RwLock::new(CPoolDescriptor {
                            weak_pool,
                            next: next_child,
                        }),
                    ));

                    arc.clone().unwrap()
                },
            )?
        };

        Ok(arc.unwrap())
    }

    fn lookup<R, F: FnOnce(Option<(&CPoolDescriptor, usize)>) -> R>(
        &self,
        caddr: CAddr,
        f: F,
    ) -> R {
        if caddr.1 == 0 {
            f(None)
        } else if caddr.1 == 1 {
            let cur_lookup_index = caddr.0[0];
            f(Some((self.read().deref(), cur_lookup_index as usize)))
        } else {
            let cur_lookup_index = caddr.0[0];
            let next_lookup_cpool: Option<CPoolCap> =
                self.read().upgrade(cur_lookup_index as usize);
            let next_caddr = caddr << 1;

            if next_lookup_cpool.is_some() {
                let next_lookup_cpool = next_lookup_cpool.unwrap();
                next_lookup_cpool.lookup::<R, F>(next_caddr, f)
            } else {
                f(None)
            }
        }
    }

    /// Lookup upgrading a capability from a capability address to a `ManagedArcAny`.
    pub fn lookup_upgrade_any(&self, caddr: CAddr) -> Option<ManagedArcAny> {
        self.lookup(caddr, |data| {
            data.map_or(None, |(cpool, index)| cpool.upgrade_any(index))
        })
    }

    /// Lookup upgrading a capability from a capability address.
    pub fn lookup_upgrade<T: Any + core::fmt::Debug>(&self, caddr: CAddr) -> Option<ManagedArc<T>> {
        self.lookup(caddr, |data| {
            data.map_or(None, |(cpool, index)| cpool.upgrade(index))
        })
    }

    /// Downgrade a capability into the capability pool at a specified capability address.
    pub fn lookup_downgrade_at<T: Any + core::fmt::Debug>(
        &self,
        arc: ManagedArc<T>,
        caddr: CAddr,
    ) -> Result<(), CapabilityErrors>
    where
        ManagedArc<T>: Any,
    {
        self.lookup(caddr, |data| {
            let (cpool, index) = data.unwrap();
            cpool.downgrade_at(arc, index)
        })
    }

    /// Downgrade a `ManagedArcAny` into the capability pool at a specified capability address.
    pub fn lookup_downgrade_any_at<T: Any>(
        &self,
        arc: ManagedArcAny,
        caddr: CAddr,
    ) -> Result<(), CapabilityErrors> {
        self.lookup(caddr, |data| {
            let (cpool, index) = data.unwrap();
            cpool.downgrade_any_at(arc, index)
        })
    }
}

#[cfg(test)]
mod tests {
    use std::mem::MaybeUninit;

    use crate::{addr::PAddrGlobal, capability::UntypedCap};

    use super::*;

    #[test]
    fn test_untyped_memory() {
        let underlying_value: Box<MaybeUninit<[u64; 40960]>> = Box::new(MaybeUninit::uninit());
        let box_addr = Box::into_raw(underlying_value) as u64;
        let addr = PAddrGlobal::new(box_addr);

        let untyped_memory = unsafe { UntypedCap::bootstrap(addr, 40960) };
        let um2 = untyped_memory.clone();

        let mut write_guard = untyped_memory.write();
        let cpool = CPoolCap::retype_from(&mut write_guard).expect("CPool cannot be created");

        cpool
            .read()
            .downgrade_any_at(um2, 0)
            .expect("Downgrade failed");

        assert_eq!(1, untyped_memory.strong_count());

        {
            let upgraded: Option<ManagedArc<RwLock<UntypedDescriptor>>> = cpool.read().upgrade(0);
            assert!(upgraded.is_some());
            assert_eq!(2, untyped_memory.strong_count());
        }
    }
}
