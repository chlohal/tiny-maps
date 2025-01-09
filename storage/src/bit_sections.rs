use std::ops::{BitAnd, BitAndAssign, BitOrAssign, Not, Shl, Shr, ShrAssign};

#[repr(transparent)]
#[derive(Copy, Clone, Debug)]
pub struct BitSection<const START: usize, const END: usize, T>(T);

pub type HighNibble = BitSection<0, 4, u8>;
pub type LowNibble = BitSection<4, 8, u8>;
pub type Byte = BitSection<0, 8, u8>;

pub type NthFirstBits<const B: usize> = BitSection<0, B, u8>;

impl<const START: usize, const END: usize, T> BitSection<START, END, T>
where
    T: Copy
        + From<u8>
        + From<bool>
        + std::ops::BitAnd<T, Output = T>
        + std::ops::BitOrAssign<T>
        + Shl<usize, Output = T>
        + Default
        + ShrAssign<usize>
        + Shr<usize, Output = T>
        + Shr<u8, Output = T>
        + Not<Output = T>
        + BitAndAssign<T>
        + BitAnd<T, Output = T>
        + BitOrAssign<T>
        + PartialEq
{
    pub fn get_bit(&self, index_from_left: usize) -> T {
        debug_assert!(index_from_left < Self::bits());

        let value = self.0 >> (std::mem::size_of::<T>() * 8 - index_from_left - 1);

        value & T::from(true)
    }
    pub fn set_bit(&mut self, index_from_left: usize, value: bool) {
        debug_assert!(index_from_left < Self::bits());

        let mask = T::from(value) << (std::mem::size_of::<T>() * 8 - index_from_left - 1);

        self.0 |= mask;
    }

    pub const fn bits() -> usize {
        END - START
    }

    pub fn mask() -> T {
        let mut mask = T::default();
        let mut i = 0;

        let high_set = T::from(true) << ((std::mem::size_of::<T>() * 8) - 1);

        while i != Self::bits() {
            i += 1;
            mask >>= 1;
            mask |= high_set;
        }

        mask >> START
    }

    pub fn copy_from(&mut self, other: T) {
        let mask = Self::mask();
        let o_mask = other & mask;
        self.0 &= !mask;
        self.0 |= o_mask;
    }
    pub fn set_range<const S: usize, const E: usize>(&mut self, other: T) {
        let reduced = self.reduce_extent_mut::<S, E>();
        
        reduced.copy_from(other)
    }
    pub fn into_inner_masked(self) -> T {
        self.0 & Self::mask()
    }
}

mod test {
    #[test]
    fn set_bit() {
        use crate::bit_sections::Byte;

        let mut byte = Byte::from(0);
        byte.set_bit(0, true);
        assert_eq!(byte.into_inner(), 0b1000_0000);

        let mut byte = Byte::from(0);
        byte.set_bit(1, true);
        assert_eq!(byte.into_inner(), 0b0100_0000);
    }

    #[test]
    fn mask() {
        use crate::bit_sections::{BitSection, HighNibble, LowNibble, Byte};

        assert_eq!(Byte::mask(), 0b1111_1111);

        assert_eq!(LowNibble::mask(), 0b1111);
        assert_eq!(HighNibble::mask(), 0b1111_0000);

        assert_eq!(BitSection::<1, 8, u8>::mask(), 0b0111_1111);
    }
}

impl<const START: usize, const END: usize, T> BitSection<START, END, T> {
    pub fn reduce_extent<const NS: usize, const NE: usize>(self) -> BitSection<NS, NE, T> {
        const {
            assert!(NS >= START);
            assert!(NE <= NE);
            assert!(NS <= NE);
        }

        BitSection(self.0)
    }

    pub fn reduce_extent_mut<const NS: usize, const NE: usize>(
        &mut self,
    ) -> &mut BitSection<NS, NE, T> {
        const {
            assert!(NS >= START);
            assert!(NE <= NE);
            assert!(NS <= NE);
        }

        unsafe {
            let f = self as *mut Self;
            let b = f.cast();
            &mut *b
        }
    }

    pub fn into_inner(self) -> T {
        self.0
    }
    pub fn from_unchecked(value: T) -> Self {

        const {
            let sz = std::mem::size_of::<T>() * 8;
            assert!( START < sz );
            assert!( END <= sz );
        }

        Self(value)
    }
}

impl<const LEAST_SIGNIFICANT_INCLUSIVE: usize, const MOST_SIGNIFICANT_EXCLUSIVE: usize, T> From<T>
    for BitSection<LEAST_SIGNIFICANT_INCLUSIVE, MOST_SIGNIFICANT_EXCLUSIVE, T>
    where
    T: Copy
        + From<u8>
        + From<bool>
        + std::ops::BitAnd<T, Output = T>
        + std::ops::BitOrAssign<T>
        + Shl<usize, Output = T>
        + Default
        + ShrAssign<usize>
        + Shr<usize, Output = T>
        + Shr<u8, Output = T>
        + Not<Output = T>
        + BitAndAssign<T>
        + BitAnd<T, Output = T>
        + BitOrAssign<T>
        + PartialEq
{
    fn from(value: T) -> Self {
        debug_assert!((value & Self::mask()) == value);

        Self::from_unchecked(value)
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
    > std::ops::BitAndAssign
    for BitSection<LEAST_SIGNIFICANT_INCLUSIVE, MOST_SIGNIFICANT_EXCLUSIVE, T>
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
    > std::ops::BitOrAssign
    for BitSection<LEAST_SIGNIFICANT_INCLUSIVE, MOST_SIGNIFICANT_EXCLUSIVE, T>
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
    > std::ops::BitXorAssign
    for BitSection<LEAST_SIGNIFICANT_INCLUSIVE, MOST_SIGNIFICANT_EXCLUSIVE, T>
{
    fn bitxor_assign(&mut self, rhs: Self) {
        self.0 ^= rhs.0;
    }
}
