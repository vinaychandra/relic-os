use core::convert::From;
use core::ops::Shl;

/// Capability address. 64bit size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct CAddr(pub [u8; 7], pub u8);

impl Shl<usize> for CAddr {
    type Output = CAddr;
    fn shl(self, rhs: usize) -> CAddr {
        assert!(rhs == 1);
        CAddr(
            [
                self.0[1], self.0[2], self.0[3], self.0[4], self.0[5], self.0[6], 0,
            ],
            self.1 - 1,
        )
    }
}

impl CAddr {
    pub fn from_u64(v: u64) -> CAddr {
        unsafe { core::mem::transmute(v) }
    }

    pub fn into_u64(self) -> u64 {
        unsafe { core::mem::transmute(self) }
    }
}

impl From<u8> for CAddr {
    fn from(v: u8) -> CAddr {
        CAddr([v, 0, 0, 0, 0, 0, 0], 1)
    }
}

impl From<[u8; 1]> for CAddr {
    fn from(v: [u8; 1]) -> CAddr {
        CAddr([v[0], 0, 0, 0, 0, 0, 0], 1)
    }
}

impl From<[u8; 2]> for CAddr {
    fn from(v: [u8; 2]) -> CAddr {
        CAddr([v[0], v[1], 0, 0, 0, 0, 0], 2)
    }
}

impl From<[u8; 3]> for CAddr {
    fn from(v: [u8; 3]) -> CAddr {
        CAddr([v[0], v[1], v[2], 0, 0, 0, 0], 3)
    }
}

impl From<[u8; 4]> for CAddr {
    fn from(v: [u8; 4]) -> CAddr {
        CAddr([v[0], v[1], v[2], v[3], 0, 0, 0], 4)
    }
}

impl From<[u8; 5]> for CAddr {
    fn from(v: [u8; 5]) -> CAddr {
        CAddr([v[0], v[1], v[2], v[3], v[4], 0, 0], 5)
    }
}

impl From<[u8; 6]> for CAddr {
    fn from(v: [u8; 6]) -> CAddr {
        CAddr([v[0], v[1], v[2], v[3], v[4], v[5], 0], 6)
    }
}

impl From<[u8; 7]> for CAddr {
    fn from(v: [u8; 7]) -> CAddr {
        CAddr([v[0], v[1], v[2], v[3], v[4], v[5], v[6]], 7)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_caddr_transmute() {
        let caddr = CAddr([0, 1, 2, 0, 0, 0, 0], 3);
        let u64: u64 = caddr.into_u64();
        let back: CAddr = CAddr::from_u64(u64);
        assert_eq!(caddr, back);
    }

    #[test]
    fn test_size() {
        assert_eq!(8, core::mem::size_of::<CAddr>());
    }
}
