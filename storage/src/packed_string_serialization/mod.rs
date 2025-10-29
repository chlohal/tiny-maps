use std::io::ErrorKind;

use is_final::IterIsFinal;

use crate::{
    bit_sections::{BitSection, Byte},
    serialize_min::{DeserializeFromMinimal, SerializeMinimal},
    varint::{from_varint, ToVarint},
};

pub mod is_final;
pub mod latin_lowercase_fivebit;
pub mod non_remainder_encodings;

use non_remainder_encodings::{
    read_some_non_remainder_encoding, try_into_some_non_remainder_encoding,
};

//to be able to serialize strings without asref()-ing them,
//which also enables serializing containers of them
impl SerializeMinimal for String {
    type ExternalData<'s> = <str as SerializeMinimal>::ExternalData<'s>;

    fn minimally_serialize<'a, 's: 'a, W: std::io::Write>(
        &'a self,
        write_to: &mut W,
        external_data: Self::ExternalData<'s>,
    ) -> std::io::Result<()> {
        self.as_str().minimally_serialize(write_to, external_data)
    }
}

/// Format of serialized strings:
/// Header byte:
/// XXXN....
///     XXX: controlled by the caller, for extra data if needed
///     N: variant. If 1, then use Fivebit encoding; if 0 then use another encoding.
///
/// Fivebit encoding:
/// XXX1VVVV
///     VVVV: used to encode the remainder, since 5 does not divide evenly into 8
///
/// Other encoding:
/// XXX01EEE -- Simple Charset encoding
///     EEE: Used to select the charset. For 4-bit charsets; this is typically for identifiers (phone numbers, web addresses, etc)
/// XXX00000 -- Empty string. No bytes follow.
/// XXX00001 -- ASCII encoding. Uses varint techniques with the last character having its high bit set
/// XXX00010 -- UTF-8 Unicode encoding. Encodes byte length + data.
///
/// XXX00011-XXX00111 are unused.
///
impl SerializeMinimal for str {
    type ExternalData<'a> = BitSection<0, 3, u8>;

    fn minimally_serialize<'a, 's: 'a, W: std::io::Write>(
        &'a self,
        write_to: &mut W,
        extra_info_nibble: Self::ExternalData<'s>,
    ) -> std::io::Result<()> {
        let value: &str = self.as_ref();

        let mut header = Byte::from(0u8 | extra_info_nibble.into_inner_masked());

        if value.is_empty() {
            //empty string! no further bytes needed past the header
            let header = header.into_inner() | 0b000;

            return write_to.write_all(&[header]);
        } 

        if let Some((nibble, bytes)) = try_into_some_non_remainder_encoding(value) {
            header.set_bit(4, true);
            let header = header.into_inner() | nibble.into_inner();
            write_to.write_all(&[header])?;

            return write_to.write_all(&bytes);
        }

        if latin_lowercase_fivebit::fits_charset(value) {
            let (nibble, bytes) = latin_lowercase_fivebit::to_charset(&value);
            
            header.set_bit(3, true);
            let header = header.into_inner() | nibble.into_inner();

            write_to.write_all(&[header])?;

            return write_to.write_all(&bytes);
        }

        if value.is_ascii() {
            let header = header.into_inner() | 0b001;

            write_to.write_all(&[header])?;

            return write_to.write_all(
                &value
                    .bytes()
                    .into_iter()
                    .is_final()
                    .map(|(f, x)| if f { x | 0b1000_0000 } else { x })
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            );
        } else {
            //UTF-8 fallback.
            let header = header.into_inner() | 0b010;

            write_to.write_all(&[header])?;

            value.as_bytes().len().write_varint(write_to)?;
            return write_to.write_all(value.as_bytes());
        }
    }
}

impl DeserializeFromMinimal for String {
    type ExternalData<'a> = Option<u8>;

    fn deserialize_minimal<'a, 'd: 'a, R: std::io::Read>(
        from: &'a mut R,
        external_data: Self::ExternalData<'d>,
    ) -> Result<Self, std::io::Error> {
        let header = match external_data {
            Some(b) => b,
            None => {
                let mut b = [0];
                from.read_exact(&mut b)?;
                b[0]
            }
        };

        //4th bit set -> fivebit encoding
        if (header >> 4) & 1 == 1 {
            let fivebit_nibble = header & 0b1111;
            return latin_lowercase_fivebit::latin_lowercase_fivebit_to_string(
                fivebit_nibble,
                from,
            );
        }

        //4th not set; 5th bit set -> non_remainder encoding
        if (header >> 3) & 1 == 1 {
            let non_remainder_nibble = header & 0b111;
            return read_some_non_remainder_encoding(non_remainder_nibble.into(), from);
        }

        //lowest 3 bits empty -> empty string
        let is_empty = header & 0b111 == 0b000;
        if is_empty {
            return Ok(String::new());
        }

        //lowest 3 bits set to 0b001 -> ASCII
        let is_ascii = header & 0b111 == 0b001;

        if is_ascii {
            let mut s = String::new();
            loop {
                let mut f = [0];
                from.read_exact(&mut f)?;
                let f = f[0];

                //if highest bit isn't set, then it's not the final bit
                if (f & 0b1000_0000) == 0 {
                    s.push(f as char);
                } else {
                    s.push((f & 0b0111_1111) as char);
                    break;
                }
            }
            return Ok(s);
        }

        //lowest 3 bits set to 0b010 -> UTF-8 unicode
        let is_utf8 = header & 0b111 == 0b010;
        if is_utf8 {
            let len = from_varint::<usize>(from)?;
            let mut buf = vec![0u8; len];
            from.read_exact(&mut buf[0..len])?;

            return String::from_utf8(buf)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e));
        }

        //none of the proper encodings triggered; therefore, complain
        return Err(ErrorKind::InvalidData.into())
    }
}
