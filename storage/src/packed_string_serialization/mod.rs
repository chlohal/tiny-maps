use std::ops::BitOrAssign;

use is_final::IterIsFinal;

use crate::{serialize_min::{DeserializeFromMinimal, SerializeMinimal}, varint::{from_varint, ToVarint}};

pub mod latin_lowercase_fivebit;
pub mod non_remainder_encodings;
pub mod is_final;

use non_remainder_encodings::{
    read_some_non_remainder_encoding, try_into_some_non_remainder_encoding,
};

#[derive(Clone, Copy)]
pub enum StringSerialVariation {
    Fivebit,
    NonRemainder,
    Ascii,
    Unicode,
}

impl SerializeMinimal for &str {
    type ExternalData<'a> = (&'a mut StringSerialVariation, &'a mut u8);

    fn minimally_serialize<'a, 's: 'a, W: std::io::Write>(
        &'a self,
        write_to: &mut W,
        extra_info_nibble: Self::ExternalData<'s>,
    ) -> std::io::Result<()> {
        let value: &str = self.as_ref();

        if let Some((nibble, bytes)) = try_into_some_non_remainder_encoding(value) {
            BitOrAssign::bitor_assign(extra_info_nibble.1, nibble);
            *extra_info_nibble.0 = StringSerialVariation::NonRemainder;

            return write_to.write_all(&bytes);
        }

        if latin_lowercase_fivebit::fits_charset(value) {
            let (nibble, bytes) = latin_lowercase_fivebit::to_charset(&value);

            BitOrAssign::bitor_assign(extra_info_nibble.1, nibble);
            *extra_info_nibble.0 = StringSerialVariation::Fivebit;

            return write_to.write_all(&bytes);
        }

        if value.is_ascii() {
            *extra_info_nibble.0 = StringSerialVariation::Ascii;

            return write_to.write_all(
                &value
                    .bytes()
                    .into_iter()
                    .is_final()
                    .map(|(f, x)| if f { x | 0b1000_0000 } else { x })
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            );
        }

        *extra_info_nibble.0 = StringSerialVariation::Unicode;

        value.as_bytes().len().write_varint(write_to)?;
        write_to.write_all(value.as_bytes())
    }
}

impl DeserializeFromMinimal for String {
    type ExternalData<'a> = &'a (StringSerialVariation, u8);

    fn deserialize_minimal<'a, 'd: 'a, R: std::io::Read>(
        from: &'a mut R,
        external_data: Self::ExternalData<'d>,
    ) -> Result<Self, std::io::Error> {
        let (codec, header_nibble) = external_data;

        match codec {
            StringSerialVariation::Fivebit => {
                latin_lowercase_fivebit::latin_lowercase_fivebit_to_string(*header_nibble, from)
            }
            StringSerialVariation::NonRemainder => {
                read_some_non_remainder_encoding(*header_nibble, from)
            }
            StringSerialVariation::Ascii => todo!(),
            StringSerialVariation::Unicode => {
                let len = from_varint::<usize>(from)?;
                let mut buf = Vec::with_capacity(len);
                from.read_exact(&mut buf[0..len])?;

                return String::from_utf8(buf)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e));
            }
        }
    }
}
