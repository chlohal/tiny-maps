use std::{
    io::Write, io::Read, mem, ops::{BitOrAssign, ShlAssign}
};

use crate::storage::serialize_min::{DeserializeFromMinimal, SerializeMinimal};

pub fn to_varint<T: ToVarint>(value: T) -> Vec<u8> {
    value.to_varint()
}

pub fn from_varint<T: FromVarint>(bytes: &mut impl Read) -> std::io::Result<T> {
    T::from_varint(bytes)
}

pub trait ToVarint {
    fn to_varint(&self) -> Vec<u8>;
}

pub trait FromVarint: Sized {
    fn from_varint(bytes: &mut impl Read) -> std::io::Result<Self>;
}

impl<T: ToVarint> SerializeMinimal for T {
    type ExternalData<'s> = ();

    fn minimally_serialize<'a, 's: 'a, W: std::io::Write>(&'a self, write_to: &mut W, external_data: Self::ExternalData<'s>) -> std::io::Result<()> {
        write_to.write_all(&self.to_varint())
    }
}

impl<T: FromVarint> DeserializeFromMinimal for T {
    type ExternalData<'a> = ();

    

    fn deserialize_minimal<'a, 'd: 'a, R: Read>(from: &'a mut R, external_data: Self::ExternalData<'d>) -> Result<Self, std::io::Error> {
        Self::from_varint(from)   
    }
}


macro_rules! impl_to_varint {
    ( $($typ:tt),* ) => {
        $(
        impl ToVarint for $typ {
            fn to_varint(&self) -> Vec<u8> {
                let mut value = *self;

                let flag_done = 0b1_000_0000;

                let mut slice = Vec::with_capacity(mem::size_of::<$typ>());

                let mask = 0b0111_1111;
                let shift = 7u8;

                loop {
                    let byte = self & mask;
                    value >>= shift;

                    slice.push(byte as u8);

                    if value == 0 {
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
        impl FromVarint for $typ {
            fn from_varint(bytes: &mut impl Read) -> std::io::Result<Self> {
                let flag_done = 0b1_000_0000;
            
                let mask = 0b1111_111u8;
                let shift = 7u8;
            
                let mut value = 0;

                let byte = 0b0u8;
            
                while let Ok(_) = bytes.read_exact(&mut [byte]) {
                    //apply byte, without value of flag
                    value |= (byte ^ flag_done) as $typ;
            
                    if (flag_done & byte) != 0 {
                        return Ok(value);
                    } else {
                        value <<= shift;
                    }
                }
            
                return Err(std::io::ErrorKind::UnexpectedEof.into());
            }
        }
    )*
    };
}

trait Zigzag {
    type Unsigned;
    fn zigzag(&self) -> Self::Unsigned;
    fn unzigzag(zigzag: Self::Unsigned) -> Self;
}

macro_rules! impl_to_be_bytes_signed_zigzag {
    ( $($typ:tt => $utyp:tt),* ) => {
        $(
        impl Zigzag for $typ {
            type Unsigned = $utyp;

            fn zigzag(&self) -> Self::Unsigned {
                (*self as $utyp).rotate_left(1)
            }
            fn unzigzag(zigzag: Self::Unsigned) -> Self {

                zigzag.rotate_right(1) as $typ
            }
        }
        impl ToVarint for $typ {
            fn to_varint(&self) -> Vec<u8> {
                Zigzag::zigzag(self).to_varint()
            }
        }
        impl FromVarint for $typ {
            fn from_varint(bytes: &mut impl Read) -> std::io::Result<Self> {
                $utyp::from_varint(bytes).map(|x| Zigzag::unzigzag(x))
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
        let result: u8 = from_varint(&mut vec![0b1_000_0000].as_slice()).unwrap();
        assert_eq!(result, 0);
    }
}
