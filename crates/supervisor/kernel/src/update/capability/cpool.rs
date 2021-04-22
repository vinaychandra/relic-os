use crate::util::boxed::{Boxed, RcRefCellBoxedInner};
use relic_abi::{cap::CapabilityErrors, prelude::CAddr};
use std::ops::Deref;

use super::*;

pub struct CPoolRead<'a> {
    data: &'a Boxed<CPoolInner>,
}

pub struct CPoolWrite<'a> {
    data: &'a mut Boxed<CPoolInner>,
}

impl Capability {
    pub fn cpool_create(&self) -> Option<CPoolRead<'_>> {
        if let Capability::CPool { data, .. } = self {
            Some(CPoolRead { data })
        } else {
            None
        }
    }

    pub fn cpool_create_mut(&mut self) -> Option<CPoolWrite<'_>> {
        if let Capability::CPool { data, .. } = self {
            Some(CPoolWrite { data })
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct CPoolInner {
    unsafe_data: [RcRefCellBoxedInner<Capability>; 256],
}

impl CPoolInner {
    pub fn store_free(&self, data: Capability) -> Result<(usize, CapRcBoxed), CapabilityErrors> {
        let mut found = false;
        let mut index: usize = 0;
        let mut result = None;
        for cell in self.unsafe_data.iter().enumerate() {
            let temp = cell.1.clone();
            result = Some(temp.clone());
            if let Ok(mut inner) = temp.try_borrow_mut() {
                *inner = data;
                index = cell.0;
                found = true;
                break;
            };
        }

        if !found {
            return Err(CapabilityErrors::CapabilitySlotsFull);
        }

        Ok((index, result.unwrap()))
    }

    pub fn lookup(&self, caddr: CAddr) -> Option<RcRefCellBoxed<Capability>> {
        if caddr.1 == 0 {
            None
        } else if caddr.1 == 1 {
            let index = caddr.0[0];
            Some(self.unsafe_data[index as usize].clone())
        } else {
            let cur_lookup_index = caddr.0[0];
            let next_lookup_cpool = &self.unsafe_data[cur_lookup_index as usize].clone();
            let borrow = next_lookup_cpool.borrow();
            if let Capability::CPool { data, .. } = &*borrow {
                let next_caddr = caddr << 1;
                data.lookup(next_caddr)
            } else {
                None
            }
        }
    }
}

impl Drop for CPoolInner {
    fn drop(&mut self) {
        todo!("CPoolInner drop not supproted")
    }
}

impl<'a> Deref for CPoolRead<'a> {
    type Target = CPoolInner;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl Capability {
    pub fn cpool_retype_from(
        untyped: &CapRcBoxed,
        cpool_to_store_in: &CPoolRead,
    ) -> Result<(CapRcBoxed, usize), CapabilityErrors> {
        let untyped_borrow = &mut *untyped.borrow_mut();
        let mut untyped_data = untyped_borrow
            .untyped_create_mut()
            .ok_or(CapabilityErrors::CapabilityMismatch)?;

        const NONE_INNER: RcRefCellBoxedInner<Capability> =
            RcRefCellBoxedInner::new(Capability::EmptyCap {
                next: None,
                prev: None,
            });

        let new = CPoolInner {
            unsafe_data: [NONE_INNER; 256],
        };

        let mut result_index = 0;
        let mut boxed2 = None;

        untyped_data.derive(|memory: *mut CPoolInner, next_child| {
            unsafe {
                core::ptr::write(memory, new);
            }
            let boxed = unsafe { Boxed::new((memory as *const CPoolInner as u64).into()) };

            let result = cpool_to_store_in.store_free(Capability::CPool {
                data: boxed,
                prev: Some(untyped.clone()),
                next: next_child.clone(),
            })?;

            if let Some(a) = &next_child {
                *a.borrow_mut().get_prev_mut() = Some(result.1.clone());
            }

            boxed2 = Some(result.1.clone());
            result_index = result.0;
            Ok(result.1)
        })?;

        Ok((boxed2.unwrap(), result_index))
    }
}
