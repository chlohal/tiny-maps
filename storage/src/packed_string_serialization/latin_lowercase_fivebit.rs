/*
    repr:
        0-25: a-z
        26: :
        27: _
        28: -
        29: /
        30: .
        31: [space]
*/

use std::io::{Error, Read};

use crate::{bit_sections::LowNibble, serialize_min::ReadExtReadOne};

use super::is_final::{Finished::*, IterIsFinal, IterTryFlatten};

pub fn fits_charset<S: AsRef<str>>(str: S) -> bool {
    return str.as_ref().chars().all(is_in_charset);
}

fn is_in_charset(ch: char) -> bool {
    let is_ascii_lower = ch.is_ascii_alphabetic() && ch.is_lowercase();

    is_ascii_lower || ch == ':' || ch == '_' || ch == ':' || ch == '/' || ch == '.' || ch == ' '
}

const LEN_REM_ONLY_0_NIBBLE: u8 = 0b0001;
const LEN_REM_0_NIBBLE: u8 = 0b0000;
const LEN_REM_ONLY_1_NIBBLE: u8 = 0b0101;
const LEN_REM_1_NIBBLE: u8 = 0b0100;
const LEN_REM_ONLY_2_NIBBLE: u8 = 0b1100;
const LEN_REM_2_NIBBLE: u8 = 0b1000;

pub fn latin_lowercase_fivebit_to_string(
    header_nibble: u8,
    mut bytes: impl Read,
) -> std::io::Result<String> {
    //if the header tells us that there's no triples, then handle
    //the case where triples shouldn't be read
    if header_nibble == LEN_REM_ONLY_0_NIBBLE {
        return Ok(String::new());
    }

    if header_nibble == LEN_REM_ONLY_1_NIBBLE {
        let byte = bytes.read_one()?;

        return String::from_utf8(vec![char_from_latin_lowercase(byte)])
            .map_err(|x| Error::new(std::io::ErrorKind::InvalidData, x));
    }

    if (header_nibble & LEN_REM_ONLY_2_NIBBLE) == LEN_REM_ONLY_2_NIBBLE {
        let a = bytes.read_one()?;

        let a_char = (header_nibble & 0b11) << 3 | (a >> 5);
        let b_char = a & 0b1_1111;

        return String::from_utf8(vec![
            char_from_latin_lowercase(a_char),
            char_from_latin_lowercase(b_char),
        ])
        .map_err(|x| Error::new(std::io::ErrorKind::InvalidData, x));
    }

    //read all the triples into a string
    let mut str = bytes
        .reading_iterator()
        .pair_chunk()
        .map_until_finished(|(a, b)| {

            let a = match a {
                Ok(a) => a,
                Err(e) => return Final(Err(e)),
            };

            let b = match b {
                Ok(b) => b,
                Err(e) => return Final(Err(e)),
            };

            let section = (a as u16) << 8 | b as u16;

            let a = char_from_latin_lowercase((section >> 11) as u8) as char;
            let b = char_from_latin_lowercase(((section >> 6) & 0b11111) as u8) as char;
            let c = char_from_latin_lowercase(((section >> 1) & 0b11111) as u8) as char;

            if section & 0b1 == 0 {
                return NonFinal(Ok([a, b, c]));
            } else {
                return Final(Ok([a, b, c]));
            }
        })
        .try_flatten()
        .collect::<std::io::Result<String>>()?;

    //read the remainder

    if header_nibble == LEN_REM_0_NIBBLE {
        return Ok(str);
    }

    if header_nibble == LEN_REM_1_NIBBLE {
        str.push(bytes.read_one()? as char);
        return Ok(str);
    }

    assert!(header_nibble == LEN_REM_2_NIBBLE);

    let a = bytes.read_one()?;

    let a_char = (header_nibble & 0b11) << 3 | (a >> 5);
    let b_char = a & 0b1_1111;

    str.push(a_char as char);
    str.push(b_char as char);

    Ok(str)
}

pub fn to_charset<'a, S: AsRef<str>>(str: &'a S) -> (LowNibble, Box<[u8]>) {
    let str = str.as_ref();

    let bytes = str.as_bytes();

    let len = bytes.len();
    let len_rem = len % 3;

    let mut header_nibble = match (len_rem, len) {
        (0, 0) => LEN_REM_ONLY_0_NIBBLE,
        (0, _) => LEN_REM_0_NIBBLE,
        (1, 1) => LEN_REM_ONLY_1_NIBBLE,
        (1, _) => LEN_REM_1_NIBBLE,
        (2, 2) => LEN_REM_ONLY_2_NIBBLE,
        (2, _) => LEN_REM_2_NIBBLE,
        _ => unreachable!(),
    };

    let remainder = if len_rem == 1 {
        Some(bytes[bytes.len() - 1])
    } else if len_rem == 2 {
        let a = char_to_latin_lowercase(bytes[bytes.len() - 2]);
        let b = char_to_latin_lowercase(bytes[bytes.len() - 1]);

        header_nibble |= a >> 3;

        Some((a << 5) | b)
    } else {
        None
    };

    (
        LowNibble::from(header_nibble),
        bytes
            .chunks_exact(3)
            .is_final()
            .map(|(is_last, chunk)| {
                let mut section = 0u16;
                let mut shift = 16 - 5;

                for b in chunk {
                    let index = char_to_latin_lowercase(*b);
                    section |= ((index & 0b1_1111) as u16) << shift;
                    shift -= 5;
                }

                if is_last {
                    section |= 1;
                }

                section.to_be_bytes()
            })
            .flatten()
            .chain(remainder)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    )
}

fn char_to_latin_lowercase(b: u8) -> u8 {
    match b {
        b':' => 26,
        b'_' => 27,
        b'-' => 28,
        b'/' => 29,
        b'.' => 30,
        b' ' => 31,
        b => b - b'a',
    }
}

fn char_from_latin_lowercase(b: u8) -> u8 {
    match b {
        26 => b':',
        27 => b'_',
        28 => b'-',
        29 => b'/',
        30 => b'.',
        31 => b' ',
        b => b'a' + b,
    }
}
