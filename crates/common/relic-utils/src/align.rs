use std::ops::{BitAnd, BitOr, Not};

use num::Num;

/// Align value downwards.
///
/// Returns the greatest x with alignment `align` so that x <= addr. The alignment must be
///  a power of 2.
#[inline]
pub fn align_down<T: Num + Copy + BitAnd<Output = T> + Not<Output = T>>(value: T, align: T) -> T {
    value & !(align - T::one())
}

/// Align value upwards.
///
/// Returns the smallest x with alignment `align` so that x >= addr. The alignment must be
/// a power of 2.
#[inline]
pub fn align_up<T: Num + Copy + BitAnd<Output = T> + BitOr<Output = T>>(value: T, align: T) -> T {
    let align_mask = align - T::one();
    if value & align_mask == T::zero() {
        value // already aligned
    } else {
        (value | align_mask) + T::one()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_align() {
        assert_eq!(8, align_up(7, 2));
        assert_eq!(8, align_up(8, 2));
        assert_eq!(8, align_up(8, 8));
        assert_eq!(16, align_up(9, 8));
        assert_eq!(16, align_up(16, 8));

        assert_eq!(6, align_down(7, 2));
        assert_eq!(8, align_down(8, 2));
        assert_eq!(8, align_down(8, 8));
        assert_eq!(8, align_down(9, 8));
        assert_eq!(16, align_down(16, 8));
    }
}
