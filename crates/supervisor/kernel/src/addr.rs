use std::{
    fmt,
    ops::{Add, AddAssign, Sub},
    ptr::NonNull,
};

macro_rules! addr_common {
    ( $t:ty, $e:expr ) => {
        impl Add<usize> for $t {
            type Output = Self;

            fn add(self, _rhs: usize) -> Self {
                Self::from(<Self as Into<usize>>::into(self) + _rhs)
            }
        }

        impl AddAssign<usize> for $t {
            fn add_assign(&mut self, _rhs: usize) {
                self.0 = ((self.0 as u64) + (_rhs as u64)) as _;
            }
        }

        impl Add<u64> for $t {
            type Output = Self;

            fn add(self, _rhs: u64) -> Self {
                Self::from(<Self as Into<u64>>::into(self) + _rhs)
            }
        }

        impl AddAssign<u64> for $t {
            fn add_assign(&mut self, _rhs: u64) {
                self.0 = ((self.0 as u64) + (_rhs as u64)) as _;
            }
        }

        impl Sub<Self> for $t {
            type Output = u64;

            fn sub(self, _rhs: Self) -> u64 {
                <Self as Into<u64>>::into(self) - <$t as Into<u64>>::into(_rhs)
            }
        }

        impl fmt::Binary for $t {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                (self.0 as u64).fmt(f)
            }
        }

        impl fmt::Display for $t {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                (self.0 as u64).fmt(f)
            }
        }

        impl fmt::LowerHex for $t {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                (self.0 as u64).fmt(f)
            }
        }

        impl fmt::Octal for $t {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                (self.0 as u64).fmt(f)
            }
        }

        impl fmt::UpperHex for $t {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                (self.0 as u64).fmt(f)
            }
        }

        impl From<usize> for $t {
            fn from(v: usize) -> Self { $e(v as _) }
        }

        impl From<u64> for $t {
            fn from(v: u64) -> Self { $e(v as _) }
        }

        impl From<u32> for $t {
            fn from(v: u32) -> Self { $e(v as _) }
        }

        impl Into<usize> for $t {
            fn into(self) -> usize { self.0 as usize }
        }

        impl Into<u64> for $t {
            fn into(self) -> u64 { self.0 as u64 }
        }

        impl Into<u32> for $t {
            fn into(self) -> u32 { self.0 as u32 }
        }

        impl<T> Into<NonNull<T>> for $t {
            fn into(self) -> NonNull<T> { NonNull::new(self.0 as u64 as _).unwrap() }
        }

        impl $t {
            pub const fn new(v: u64) -> $t { $e(v as _) }
        }
    }
}

/// Represent a physical memory address.
#[derive(Copy, Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PAddr(*const ());
unsafe impl Sync for PAddr {}
unsafe impl Send for PAddr {}

addr_common!(PAddr, PAddr);

/// Represent a physical memory address in kernel address space.
#[derive(Copy, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct PAddrGlobal(*const ());
unsafe impl Sync for PAddrGlobal {}
unsafe impl Send for PAddrGlobal {}

impl core::fmt::Debug for PAddrGlobal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("PAddrGlobal")
            .field(&self.to_paddr())
            .finish()
    }
}

addr_common!(PAddrGlobal, PAddrGlobal);

/// Represent a virtual memory address in some address space.
#[derive(Copy, Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct VAddr(*const ());
unsafe impl Sync for VAddr {}
unsafe impl Send for VAddr {}

addr_common!(VAddr, VAddr);

impl PAddrGlobal {
    #[cfg(test)]
    pub fn to_paddr(&self) -> PAddr {
        PAddr(self.0)
    }

    #[cfg(not(test))]
    pub fn to_paddr(&self) -> PAddr {
        PAddr(((self.0 as u64) - crate::arch::globals::MEM_MAP_OFFSET_LOCATION) as _)
    }

    pub unsafe fn as_mut_ptr<T>(&self) -> &mut T {
        &mut *(self.0 as *mut T)
    }
}

impl PAddr {
    #[cfg(test)]
    pub fn to_paddr_global(&self) -> PAddrGlobal {
        PAddrGlobal(self.0)
    }

    #[cfg(not(test))]
    pub fn to_paddr_global(&self) -> PAddrGlobal {
        PAddrGlobal(((self.0 as u64) + crate::arch::globals::MEM_MAP_OFFSET_LOCATION) as _)
    }
}

impl VAddr {
    pub unsafe fn as_mut_ptr<T>(&self) -> &mut T {
        &mut *(self.0 as *mut T)
    }
}
