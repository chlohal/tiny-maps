use std::{
    io::Write, io::Read, mem, ops::{BitOrAssign, ShlAssign}
};

use crate::storage::serialize_min::{DeserializeFromMinimal, SerializeMinimal, ReadExtReadOne};

pub fn to_varint<T: ToVarint>(value: T) -> Vec<u8> {
    value.to_varint()
}

pub fn from_varint<T: FromVarint>(bytes: &mut impl Read) -> std::io::Result<T> {
    T::from_varint(bytes)
}

pub trait ToVarint {
    fn write_varint(&self, to: &mut impl std::io::Write) -> std::io::Result<()>;
    fn to_varint(&self) -> Vec<u8> {
        let mut v = Vec::new();
        self.write_varint(&mut v).unwrap();
        v
    }
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
            fn write_varint(&self, to: &mut impl std::io::Write) -> std::io::Result<()> {
                let mut value = *self;

                let flag_more = 0b1_000_0000;
                let bits_per_byte = 7;

                //round the shift down to a multiple of bits_per_byte
                let mut shift = ($typ::BITS - self.leading_zeros());
                shift = shift - (shift % bits_per_byte);

                let mut mask: $typ = 0b_111_1111 << shift;

                loop {
                    let byte = (self & mask) >> shift;
                    value >>= bits_per_byte;
                    mask >>= bits_per_byte;
                    shift = shift.saturating_sub(bits_per_byte);

                    if value == 0 {
                        to.write_all(&[(byte as u8)])?;
                        break;
                    } else {
                        to.write_all(&[(byte as u8) | flag_more])?;
                    }
                }

                Ok(())
            }
        }
        impl FromVarint for $typ {
            fn from_varint(bytes: &mut impl Read) -> std::io::Result<Self> {
                let flag_more = 0b1_000_0000;
            
                let shift = 7u8;
            
                let mut value = 0;
            
                for byte in bytes.reading_iterator() {
                    let byte = byte?;

                    //apply byte, without value of flag
                    value |= (byte & !flag_more) as $typ;
            
                    if (flag_more & byte) == 0 {
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
            fn write_varint(&self, to: &mut impl std::io::Write) -> std::io::Result<()> {
                Zigzag::zigzag(self).write_varint(to)
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

    #[test]
    fn roundtrip() {
        let value = 587321u64;

        let mut to = &to_varint(value)[..];
        assert_eq!(from_varint::<u64>(&mut to).unwrap(), value);
    }
}
