use osmpbfreader::Tags;

use super::{packed_strings::PackedString, LiteralPool, OsmLiteralSerializable};


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
    SplitCommaList(Vec<LiteralValue>),
    String(String),
    AsciiString(String),
    LowerAsciiString(String),
}

impl LiteralValue {
    pub fn as_number(&self) -> Option<isize> {
        match self {
            LiteralValue::UInt(num) => Some(*num as isize),
            LiteralValue::IInt(num) => Some(*num as isize),
            LiteralValue::TinyUNumber(num) => Some(*num as isize),
            LiteralValue::TinyINumber(num) => Some(*num as isize),
            LiteralValue::String(s)
            | LiteralValue::AsciiString(s)
            | LiteralValue::LowerAsciiString(s) => {
                s.parse().ok()
            },
            _ => None
        }
    }
    
    pub fn from_tag_and_remove(tags: &mut Tags, key: &str) -> Option<Self> {
        let value = tags.remove(key)?;

        return Some(LiteralValue::from(value));
    }
}

impl From<PackedString> for LiteralValue {
    fn from(value: PackedString) -> Self {
        match value {
            PackedString::LowerLatinUnderscoreHyphenColonFiveBit(_) => todo!(),
            PackedString::CasedLatinUnderscoreHyphenColonSixBit(_) => todo!(),
            PackedString::Ascii(a) => LiteralValue::AsciiString(a.into_iter().map(|x| (x ^ 0b1000_0000) as char).collect()),
            PackedString::Unicode(s) => LiteralValue::String(s),
        }
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
            }
            else if num >= -0b1_1111 && num < 0b1_1111 {
                return LiteralValue::TinyINumber(num as i8);
            } else if num >= 0 {
                return LiteralValue::UInt(num as u64);
            } else {
                return LiteralValue::IInt(num)
            }
        }

        if value.len() == 2 && is_ascii_upper_alpha(value) {
            let a = value.as_bytes()[0];
            let b = value.as_bytes()[1];
            return LiteralValue::TwoUpperLatinAbbrev(a, b);
        }

        if value.is_ascii() {
            if is_ascii_lower_alpha(value) {
                return LiteralValue::LowerAsciiString(value.to_string());
            }
            return LiteralValue::AsciiString(value.to_string());
        }

        return LiteralValue::String(value.to_string())
    }
}

fn is_ascii_lower_alpha(s: &str) -> bool {
    for ch in s.chars() {
        if !ch.is_ascii_lowercase() || !ch.is_ascii_alphabetic() {
            return false;
        }
    }
    return true;
}

fn is_ascii_upper_alpha(s: &str) -> bool {
    for ch in s.chars() {
        if !ch.is_ascii_uppercase() || !ch.is_ascii_alphabetic() {
            return false;
        }
    }
    return true;
}

impl OsmLiteralSerializable for LiteralValue {
    type Category = LiteralValue;
    
    fn serialize_to_pool(&self, pool: &mut LiteralPool<LiteralValue>) -> Vec<u8> {
        todo!()
    }
    
}