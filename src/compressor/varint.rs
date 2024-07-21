use std::{
    mem,
    ops::{BitOrAssign, ShlAssign},
};

pub fn to_varint<T: ToVarint>(value: T) -> Vec<u8> {
    value.to_varint()
}

pub fn from_varint<T: Default + BitOrAssign<u8> + ShlAssign<u8>>(bytes: &[u8]) -> Option<T> {
    let flag_done = 0b1_000_0000;

    let mask = 0b1111_111u8;
    let shift = 7u8;

    let mut value = T::default();

    for byte in bytes {
        //apply byte, without value of flag
        value |= byte ^ flag_done;

        if flag_done & byte != 0 {
            return Some(value);
        } else {
            value <<= shift;
        }
    }

    return None;
}

trait ToVarint {
    fn to_varint(self) -> Vec<u8>;
}

macro_rules! impl_to_varint {
    ( $($typ:tt),* ) => {
        $(
        impl ToVarint for $typ {
            fn to_varint(mut self) -> Vec<u8> {
                let flag_done = 0b1_000_0000;

                let mut slice = Vec::with_capacity(mem::size_of::<$typ>());

                let mask = 0b0111_1111;
                let shift = 7u8;

                loop {
                    let byte = self & mask;
                    self >>= shift;

                    slice.push(byte as u8);

                    if self == 0 {
                        break;
                    }
                }

                //put MSB first
                slice.reverse();
                slice.shrink_to_fit();

                //stick the Done flag on the last byte
                *slice.last_mut().unwrap() |= flag_done;

                slice
            }
        }
    )*
    };
}

trait Zigzag {
    type Unsigned;
    fn zigzag(self) -> Self::Unsigned;
}

macro_rules! impl_to_be_bytes_signed_zigzag {
    ( $($typ:tt => $utyp:tt),* ) => {
        $(
        impl Zigzag for $typ {
            type Unsigned = $utyp;

            fn zigzag(self) -> Self::Unsigned {
                let mut zigzag = self as $utyp;
                let ibit = zigzag >> $utyp::BITS - 1;
                zigzag <<= 1;
                zigzag |= ibit;

                zigzag
            }
        }
        impl ToVarint for $typ {
            fn to_varint(self) -> Vec<u8> {
                Zigzag::zigzag(self).to_varint()
            }
        }
    )*
    };
}

impl_to_varint! {u8, u16, u32, u64, u128, usize}
impl_to_be_bytes_signed_zigzag! {i8 => u8, i16 => u16, i32 => u32, i64 => u64, i128 => u128}

#[cfg(test)]
mod tests {
    use crate::compressor::varint::*;

    #[test]
    fn encode() {
        let result = to_varint(0);
        assert_eq!(result, vec![0b1_000_0000]);
    }

    #[test]
    fn decode() {
        let result: u8 = from_varint(&vec![0b1_000_0000]).unwrap();
        assert_eq!(result, 0);
    }
}
