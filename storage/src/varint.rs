use std::io::{Read, Write};

use crate::serialize_min::{DeserializeFromMinimal, ReadExtReadOne, SerializeMinimal};

pub fn to_varint<T: ToVarint>(value: T) -> Vec<u8> {
    value.to_varint()
}

pub fn from_varint<T: FromVarint>(bytes: &mut impl Read) -> std::io::Result<T> {
    T::from_varint(bytes)
}

pub trait ToVarint {
    fn write_varint(&self, to: &mut impl Write) -> std::io::Result<()>;
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

    fn minimally_serialize<'a, 's: 'a, W: Write>(
        &'a self,
        write_to: &mut W,
        _external_data: (),
    ) -> std::io::Result<()> {
        write_to.write_all(&self.to_varint())
    }
}

impl<T: FromVarint> DeserializeFromMinimal for T {
    type ExternalData<'a> = ();

    fn deserialize_minimal<'a, 'd: 'a, R: Read>(
        from: &'a mut R,
        _external_data: (),
    ) -> Result<Self, std::io::Error> {
        Self::from_varint(from)
    }
}

macro_rules! impl_to_varint {
    ( $($typ:tt),* ) => {
        $(
        impl ToVarint for $typ {
            fn write_varint(&self, to: &mut impl std::io::Write) -> std::io::Result<()> {
                let flag_more = 0b1_000_0000;
                let bits_per_byte = 7;

                let mut shift = ($typ::BITS - self.leading_zeros());

                shift = (shift / bits_per_byte) * bits_per_byte;

                

                loop {
                    let mask: $typ = 0b_111_1111 << shift;
                    let byte = (self & mask) >> shift;

                    if shift == 0 {
                        to.write_all(&[(byte as u8)])?;
                        return Ok(());
                    } else {
                        shift = shift.saturating_sub(bits_per_byte);
                        
                        to.write_all(&[(byte as u8) | flag_more])?;
                    }
                }
            }
        }
        impl FromVarint for $typ {
            fn from_varint(bytes: &mut impl Read) -> std::io::Result<Self> {
                let flag_more = 0b1_000_0000;

                let shift = 7u8;

                let mut value = 0;

                loop {
                    let byte = bytes.read_one()?;

                    //apply byte, without value of flag
                    value |= (byte & !flag_more) as $typ;

                    if (flag_more & byte) == 0 {
                        return Ok(value);
                    }
                    value <<= shift;
                }
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
impl_to_be_bytes_signed_zigzag! {i8 => u8, i16 => u16, i32 => u32, i64 => u64, i128 => u128, isize => usize}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode() {
        let result = to_varint(0);
        assert_eq!(result, vec![0b1_000_0000]);
    }

    #[test]
    fn decode() {
        let result: u8 = from_varint(&mut vec![0b0_000_0000].as_slice()).unwrap();
        assert_eq!(result, 0);
    }

    #[test]
    fn adverse_roundtrip() {
        let value = 128;

        dbg!(to_varint(value));

        let mut to = &to_varint(value)[..];
        assert_eq!(from_varint::<u64>(&mut to).unwrap(), value);
    }

    #[test]
    fn minmax() {
        macro_rules! make_limit_test {
            ($val:expr, $($t:tt)*) => {
                {
                    let value = $val;
                    let to = to_varint(value);
                    let out = from_varint(&mut &to[..]).unwrap();
                    if(value != out) {
                        eprintln!("{:?}", to);
                        assert_eq!(value, out, $($t)*);
                    }
                }
            };
        }

        macro_rules! make_minmax_test {
            ($( $typ:ty ),*) => {
                $(
                    make_limit_test!(<$typ>::MAX, "maximum of {} should survive roundtrip varints", stringify!($typ));
                    make_limit_test!(<$typ>::MIN, "minimum of {} should survive roundtrip varints", stringify!($typ));
                )*
            };
        }
        make_minmax_test!(u8, i8, u16, i16, u32, i32, u64, i64);
    }

    #[test]
    fn roundtrip() {
        for value in [
            1,
            2,
            4,
            8,
            16,
            32,
            64,
            128,
            256,
            142361806282472958u64,
            6791104u64,
            39406836u64,
            17391677u64,
            4796168u64,
            148478827u64,
            5703434u64,
            2026716u64,
            16612077u64,
            21815112u64,
            25611391u64,
            50736485u64,
            145740861u64,
            15962560u64,
            7512008u64,
            62085279u64,
            142461646u64,
            8125243u64,
            27030150u64,
            12038051u64,
            16506797u64,
            1454362439u64,
            24122395u64,
            31770804u64,
            3632437u64,
            151495884u64,
            3539001u64,
            41138433u64,
            209021241u64,
            4009362u64,
            6166955u64,
            386708171u64,
            63864899u64,
            11287631u64,
            1645593u64,
            2592461u64,
            22285206u64,
            62192392u64,
            37433174u64,
            9810054u64,
            5631421u64,
            2931019u64,
            94732639u64,
            31287186u64,
            102597093u64,
            30068762u64,
            15248553u64,
            21227468u64,
            5188914u64,
            54738497u64,
            40546372u64,
            20332593u64,
            252899588u64,
            54391102u64,
            797344187u64,
            1603410060u64,
            1418367550u64,
            460978379u64,
            107041910u64,
            99933461u64,
            12656623u64,
            11977039u64,
            354395629u64,
            27319534u64,
            2970785u64,
            274430u64,
            3499419u64,
            109323045u64,
        ] {
            let mut to = &to_varint(value)[..];
            assert_eq!(from_varint::<u64>(&mut to).unwrap(), value);
        }
    }
}
