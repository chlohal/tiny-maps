use std::io::Read;

use osmpbfreader::Tags;

use crate::compressor::varint::ToVarint;
use crate::storage::serialize_min::DeserializeFromMinimal;
use crate::storage::serialize_min::SerializeMinimal;
use crate::compressor::varint::{from_varint};
use crate::storage::serialize_min::ReadExtReadOne;

use super::packed_strings::StringSerialVariation;
use super::LiteralPool;

#[derive(Debug, Clone)]
pub enum LiteralValue {
    BoolYes,
    BoolNo,
    Blank,
    UInt(u64),
    IInt(i64),
    TinyUNumber(u8),
    TinyINumber(i8),
    Date(usize, usize, usize),
    Time(usize, usize),
    ListWithSep(u8, Vec<LiteralValue>),
    TwoUpperLatinAbbrev(u8, u8),
    SplitSemiList(Vec<LiteralValue>),

    String(String), //NOTE: THIS TAKES UP THE EQUIVALENT OF 4 VARIANTS IN THE BINARY REPRESENTATION

    Ref(u64),
}

impl LiteralValue {
    pub fn as_number(&self) -> Option<isize> {
        match self {
            LiteralValue::UInt(num) => Some(*num as isize),
            LiteralValue::IInt(num) => Some(*num as isize),
            LiteralValue::TinyUNumber(num) => Some(*num as isize),
            LiteralValue::TinyINumber(num) => Some(*num as isize),
            LiteralValue::String(s) => s.parse().ok(),
            _ => None,
        }
    }

    pub fn from_tag_and_remove(tags: &mut Tags, key: &str) -> Option<Self> {
        let value = tags.remove(key)?;

        return Some(LiteralValue::from(value));
    }
}

impl<T: AsRef<str>> From<T> for LiteralValue {
    fn from(value: T) -> Self {
        let value = value.as_ref();

        match value {
            "" => return LiteralValue::Blank,
            "yes" => return LiteralValue::BoolYes,
            "no" => return LiteralValue::BoolNo,
            _ => {}
        }

        if let Ok(num) = value.parse::<i64>() {
            if num >= 0 && num < 0b11_1111 {
                return LiteralValue::TinyUNumber(num as u8);
            } else if num >= -0b1_1111 && num < 0b1_1111 {
                return LiteralValue::TinyINumber(num as i8);
            } else if num >= 0 {
                return LiteralValue::UInt(num as u64);
            } else {
                return LiteralValue::IInt(num);
            }
        }

        if value.len() == 2 && is_ascii_upper_alpha(value) {
            let a = value.as_bytes()[0];
            let b = value.as_bytes()[1];
            return LiteralValue::TwoUpperLatinAbbrev(a, b);
        }

        return LiteralValue::String(value.to_string());
    }
}

fn is_ascii_upper_alpha(s: &str) -> bool {
    for ch in s.chars() {
        if !ch.is_ascii_uppercase() || !ch.is_ascii_alphabetic() {
            return false;
        }
    }
    return true;
}

impl DeserializeFromMinimal for LiteralValue {
    type ExternalData<'a> = ();

    

    fn deserialize_minimal<'a, 'd: 'a, R: Read>(from: &'a mut R, external_data: ()) -> Result<Self, std::io::Error> {
        let header_byte = from.read_one()?;

        let enum_variant = header_byte >> 4;
        let lower_nibble_ext_info = header_byte & 0b1111;

        let res = match enum_variant {
            0b0000 => LiteralValue::BoolYes,
            0b0001 => LiteralValue::BoolNo,
            0b0010 => LiteralValue::Blank,
            0b0011 => LiteralValue::UInt(from_varint(from)?),
            0b0100 => LiteralValue::IInt(from_varint(from)?),
            0b0101 => LiteralValue::TinyUNumber(lower_nibble_ext_info),
            0b0110 => LiteralValue::TinyINumber((lower_nibble_ext_info & 0b111) as i8 * if lower_nibble_ext_info & 0b1000 != 0 { -1 } else { 1 } ),
            0b0111 => todo!(), // LiteralValue::Date(_, _, _)
            0b1000 => todo!(), // LiteralValue::Time(_, _)
            0b1001 => todo!(), // LiteralValue::ListWithSep(_, _)
            0b1010 => LiteralValue::TwoUpperLatinAbbrev(from.read_one()?, from.read_one()?),
            0b1011 => todo!(), // LiteralValue::SplitSemiList(_)
            0b1100 | 0b1101 | 0b1110 | 0b1111 => {
                let variation = offset_to_string_value_enum(enum_variant & 0b11)?;
                
                LiteralValue::String(String::deserialize_minimal(from, &mut (variation, lower_nibble_ext_info))?)
            }, 
            _ => unreachable!()
        };

        Ok(res)
    }
}
impl SerializeMinimal for LiteralValue {
    type ExternalData<'s> = ();

    fn minimally_serialize<'a, 's:'a, W: std::io::Write>(&'a self, write_to: &mut W, _external_data: ()) -> std::io::Result<()> {
        let mut buf = Vec::new();
        buf.push(0u8);

        let header_byte = match self {
            LiteralValue::BoolYes => 0b0000_0000u8,
            LiteralValue::BoolNo => 0b0001_0000u8,
            LiteralValue::Blank => 0b0010_0000u8,
            LiteralValue::UInt(num) => {
                num.write_varint(&mut buf)?;
                0b0011_0000u8
            }
            LiteralValue::IInt(num) => {
                num.write_varint(&mut buf)?;
                0b0100_0000u8
            }
            LiteralValue::TinyUNumber(num) => 0b0101_0000u8 | *num,
            LiteralValue::TinyINumber(num) => 0b0110_0000u8 | (num.unsigned_abs() | if *num < 0 { 0b1000 } else { 0b0000 }),
            LiteralValue::Date(_, _, _) => todo!(), // 0b0111_0000u8
            LiteralValue::Time(_, _) => todo!(),    // 0b1000_0000u8
            LiteralValue::ListWithSep(_, _) => todo!(), // 0b1001_0000u8
            LiteralValue::TwoUpperLatinAbbrev(a, b) => {
                buf.push(*a);
                buf.push(*b);
                0b1010_0000u8
            }
            LiteralValue::SplitSemiList(_) => todo!(), // 0b1011_0000u8
            LiteralValue::String(s) => {
                let variety = StringSerialVariation::Unicode;
                let low_nibble = 0;

                s.minimally_serialize(&mut buf, &mut (variety, low_nibble))?;

                ((0b11_00 + string_value_enum_offset(variety)) << 4) | low_nibble

            },

            LiteralValue::Ref(_) => panic!("Unable to serialize a reference!"),
        };

        buf[0] = header_byte;
        
        write_to.write_all(&buf)
    }
}

fn string_value_enum_offset(variety: StringSerialVariation) -> u8 {
    match variety {
        StringSerialVariation::Fivebit => 0,
        StringSerialVariation::NonRemainder => 1,
        StringSerialVariation::Ascii => 2,
        StringSerialVariation::Unicode => 3,
    }
}

fn offset_to_string_value_enum(val: u8) -> std::io::Result<StringSerialVariation> {
    match val {
        0 => Ok(StringSerialVariation::Fivebit),
        1 => Ok(StringSerialVariation::NonRemainder),
        2 => Ok(StringSerialVariation::Ascii),
        3 => Ok(StringSerialVariation::Unicode),
        _ => Err(std::io::ErrorKind::InvalidData.into())
    }
}