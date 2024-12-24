use std::io::ErrorKind;

use crate::{bit_sections::{BitSection, LowNibble}, serialize_min::ReadExtReadOne};

pub mod fourbits;

pub const NIBBLE_PHONENUMBER_FOURBIT: u8 = 0b000;
pub const NIBBLE_MOSTLY_NUMERIC_FOURBIT: u8 = 0b001;
pub const NIBBLE_NUM_IDENTIFIER_FOURBIT: u8 = 0b010;

pub const NIBBLE_ONE_CHAR_BYTE: u8 = 0b100;

pub fn try_into_some_non_remainder_encoding(str: &str) -> Option<(BitSection<5, 8, u8>, Box<[u8]>)> {
    if str.is_ascii() && str.len() == 1 {
        return Some((
            NIBBLE_ONE_CHAR_BYTE.into(),
            vec![str.as_bytes()[0]].into_boxed_slice(),
        ));
    }

    if let Some(fourbit) = fourbits::mostly_numeric_fourbit::from_string(&str) {
        return Some((NIBBLE_MOSTLY_NUMERIC_FOURBIT.into(), fourbit));
    }

    if let Some(fourbit) = fourbits::phonenumber_fourbit::from_string(&str) {
        return Some((NIBBLE_PHONENUMBER_FOURBIT.into(), fourbit));
    }

    if let Some(fourbit) = fourbits::numeric_identifier_fourbit::from_string(&str) {
        return Some((NIBBLE_NUM_IDENTIFIER_FOURBIT.into(), fourbit));
    }

    return None;
}

pub fn read_some_non_remainder_encoding(header_nibble: BitSection<5, 8, u8>, from: &mut impl std::io::Read) -> std::io::Result<String> {
    let header_nibble = header_nibble.into_inner_masked();

    match header_nibble {
        NIBBLE_ONE_CHAR_BYTE => String::from_utf8(vec![from.read_one()?]).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
        NIBBLE_MOSTLY_NUMERIC_FOURBIT => fourbits::mostly_numeric_fourbit::to_string(from),
        NIBBLE_PHONENUMBER_FOURBIT => fourbits::phonenumber_fourbit::to_string(from),
        NIBBLE_NUM_IDENTIFIER_FOURBIT => fourbits::numeric_identifier_fourbit::to_string(from),
        _ => Err(std::io::Error::new(ErrorKind::InvalidData, "Unable to read any applicable non-remainder encoding"))
    }
}