use std::ops::{BitAndAssign, BitOrAssign, BitXor};

#[repr(transparent)]
pub struct BitSection<
    const START: usize,
    const END: usize,
    T,
>(T);

pub type HighNibble = BitSection<0, 4, u8>;
pub type LowNibble = BitSection<4, 8, u8>;
pub type Byte = BitSection<0, 8, u8>;

pub type NthFirstBits<const B: usize> = BitSection<0, B, u8>;

impl<
const START: usize,
const END: usize> BitSection<START, END, u8> {
    pub fn set_bit(&mut self, index_from_left: usize, value: bool) {
        debug_assert!(index_from_left < Self::bits());

        let mask = (value as u8) << (std::mem::size_of::<u8>() * 8 - index_from_left - 1);

        self.0 |= mask;
    }

    pub const fn bits() -> usize {
        END - START
    }

    pub const fn mask() -> Self {
        let mut mask = 0;
        let mut i = 0;

        while i != Self::bits() {
            i += 1;
            mask >>= 1;
            mask |= 0b1000_0000;
        }

        Self(mask >> START)
    }

    pub fn copy_from(&mut self, other: u8) {
        let mask = Self::mask().into_inner();
        let o_mask = other & mask;
        self.0 &= !mask;
        self.0 |= o_mask;
    }
    pub fn set_range<const S: usize, const E: usize>(&mut self, other: u8) {
        self.reduce_extent_mut::<S, E>().copy_from(other)
    }
}

mod test {
    use crate::bit_sections::{BitSection, HighNibble, LowNibble};

    use super::Byte;

    #[test]
    fn set_bit() {
        let mut byte = Byte::from(0);
        byte.set_bit(0, true);
        assert_eq!(byte.into_inner(), 0b1000_0000);

        let mut byte = Byte::from(0);
        byte.set_bit(1, true);
        assert_eq!(byte.into_inner(), 0b0100_0000);
    }

    #[test]
    fn mask() {
        assert_eq!(Byte::mask().into_inner(), 0b1111_1111);

        assert_eq!(LowNibble::mask().into_inner(), 0b1111);
        assert_eq!(HighNibble::mask().into_inner(), 0b1111_0000);

        assert_eq!(BitSection::<1, 8, u8>::mask().into_inner(), 0b0111_1111);
    }
}

impl<
const START: usize,
const END: usize,
T
> BitSection<START, END, T> {
    pub fn reduce_extent<const NS: usize, const NE: usize>(self) -> BitSection<NS, NE, T> {
        debug_assert!(NS >= START);
        debug_assert!(NE <= NE);
        debug_assert!(NS <= NE);

        BitSection(self.0)
    }

    pub fn reduce_extent_mut<const NS: usize, const NE: usize>(&mut self) -> &mut BitSection<NS, NE, T> {
        debug_assert!(NS >= START);
        debug_assert!(NE <= NE);
        debug_assert!(NS <= NE);

        unsafe {
            let f = self as *mut Self;
            let b = f.cast();
            &mut *b
        }
    }

    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<
const LEAST_SIGNIFICANT_INCLUSIVE: usize,
const MOST_SIGNIFICANT_EXCLUSIVE: usize,
T
> From<T> for BitSection<LEAST_SIGNIFICANT_INCLUSIVE, MOST_SIGNIFICANT_EXCLUSIVE, T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}



impl<
        const LEAST_SIGNIFICANT_INCLUSIVE: usize,
        const MOST_SIGNIFICANT_EXCLUSIVE: usize,
        T: std::ops::BitAnd<Output = T>,
    > std::ops::BitAnd for BitSection<LEAST_SIGNIFICANT_INCLUSIVE, MOST_SIGNIFICANT_EXCLUSIVE, T>
{
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl<
        const LEAST_SIGNIFICANT_INCLUSIVE: usize,
        const MOST_SIGNIFICANT_EXCLUSIVE: usize,
        T: std::ops::BitAndAssign,
    > std::ops::BitAndAssign for BitSection<LEAST_SIGNIFICANT_INCLUSIVE, MOST_SIGNIFICANT_EXCLUSIVE, T>
{
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl<
        const LEAST_SIGNIFICANT_INCLUSIVE: usize,
        const MOST_SIGNIFICANT_EXCLUSIVE: usize,
        T: std::ops::BitOr<Output = T>,
    > std::ops::BitOr for BitSection<LEAST_SIGNIFICANT_INCLUSIVE, MOST_SIGNIFICANT_EXCLUSIVE, T>
{
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl<
        const LEAST_SIGNIFICANT_INCLUSIVE: usize,
        const MOST_SIGNIFICANT_EXCLUSIVE: usize,
        T: std::ops::BitOrAssign,
    > std::ops::BitOrAssign for BitSection<LEAST_SIGNIFICANT_INCLUSIVE, MOST_SIGNIFICANT_EXCLUSIVE, T>
{
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl<
        const LEAST_SIGNIFICANT_INCLUSIVE: usize,
        const MOST_SIGNIFICANT_EXCLUSIVE: usize,
        T: std::ops::BitXor<Output = T>,
    > std::ops::BitXor for BitSection<LEAST_SIGNIFICANT_INCLUSIVE, MOST_SIGNIFICANT_EXCLUSIVE, T>
{
    type Output = Self;

    fn bitxor(self, rhs: Self) -> Self::Output {
        Self(self.0 ^ rhs.0)
    }
}

impl<
        const LEAST_SIGNIFICANT_INCLUSIVE: usize,
        const MOST_SIGNIFICANT_EXCLUSIVE: usize,
        T: std::ops::BitXorAssign,
    > std::ops::BitXorAssign for BitSection<LEAST_SIGNIFICANT_INCLUSIVE, MOST_SIGNIFICANT_EXCLUSIVE, T>
{
    fn bitxor_assign(&mut self, rhs: Self) {
        self.0 ^= rhs.0;
    }
}

