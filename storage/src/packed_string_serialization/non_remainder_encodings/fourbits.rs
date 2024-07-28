/*
    repr:
        0-9: 0-9
        10: Q
        11: (
        12: -
        13: [space]
        14: )
        15: END
*/
macro_rules! impl_fourbit_encoding {
    ($($ch:literal => $enc:literal),* ) => {

        use crate::serialize_min::ReadExtReadOne;
        use crate::packed_string_serialization::is_final::IterChunksPadded;

    pub fn fits_charset<S: AsRef<str>>(str: S) -> bool {
        return str.as_ref().chars().all(is_in_charset);
    }

    fn is_in_charset(ch: char) -> bool {
        char_to_charset(ch).is_some()
    }

    fn char_to_charset(b: char) -> Option<u8> {
        match b {
            $(
                $ch => Some($enc),
            )*
            _ => None,
        }
    }

    fn charset_to_char(b: u8) -> Option<char> {
        match b {
            $(
                $enc => Some($ch),
            )*
            _ => None,
        }
    }

    pub fn to_string(bytes: &mut impl std::io::Read) -> std::io::Result<String> {
        let mut str = String::new();

        loop {
            let byte = bytes.read_one()?;
            let a = byte >> 4;
            let b = byte & 0b1111;

            let Some(a_char) = charset_to_char(a) else {
                break;
            };
            str.push(a_char);

            let Some(b_char) = charset_to_char(b) else {
                break;
            };
            str.push(b_char)
        }

        Ok(str)
    }

    pub fn from_string<'a, S: AsRef<str>>(str: &'a S) -> Option<Box<[u8]>> {
        let str = str.as_ref();

        let bytes = str.as_bytes();

        let mut value = Vec::with_capacity(bytes.len() / 2 + 1);

        for pair in bytes.chunks_padded(2) {
            let a = pair[0];
            let b = pair[1];

            let nibble_a = (if a == 0 {
                Some(0b1111)
            } else {
                char_to_charset(a as char)
            })?;

            let nibble_b = (if b == 0 {
                Some(0b1111)
            } else {
                char_to_charset(b as char)
            })?;

            value.push((nibble_a << 4) | nibble_b);
        }

        Some(value.into_boxed_slice())
    }

};
}

pub mod phonenumber_fourbit {
impl_fourbit_encoding! {
    '0' => 0,
    '1' => 1,
    '2' => 2,
    '3' => 3,
    '4' => 4,
    '5' => 5,
    '6' => 6,
    '7' => 7,
    '8' => 8,
    '9' => 9,
    '(' => 10,
    ')' => 11,
    '+' => 12,
    ' ' => 13,
    '-' => 14
}}

pub mod mostly_numeric_fourbit {
impl_fourbit_encoding! {
    '0' => 0,
    '1' => 1,
    '2' => 2,
    '3' => 3,
    '4' => 4,
    '5' => 5,
    '6' => 6,
    '7' => 7,
    '8' => 8,
    '9' => 9,
    ',' => 10,
    '-' => 11,
    ' ' => 12,
    '/' => 13,
    '.' => 14
}}

pub mod numeric_identifier_fourbit {
impl_fourbit_encoding! {
    '0' => 0,
    '1' => 1,
    '2' => 2,
    '3' => 3,
    '4' => 4,
    '5' => 5,
    '6' => 6,
    '7' => 7,
    '8' => 8,
    '9' => 9,
    'Q' => 10,
    'a' => 11,
    'b' => 12,
    ' ' => 13,
    '.' => 14
}}
