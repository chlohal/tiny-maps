use super::{literal_value::LiteralValue, LiteralPool, OsmLiteralSerializable};


#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub enum PackedString {
    LowerLatinUnderscoreHyphenColonFiveBit(Box<[u8]>),
    CasedLatinUnderscoreHyphenColonSixBit(Box<[u8]>),
    Ascii(Box<[u8]>),
    Unicode(String),
}


impl<T: AsRef<str>> From<T> for PackedString {
    fn from(value: T) -> Self {
        let value = value.as_ref();
        let len = value.as_bytes().len();

        if value.is_ascii() {
            let last_index = len - 1;
            Self::Ascii(value.bytes().into_iter().enumerate().map(|(i, x)| {
                if i == last_index {
                    x | 0b1000_0000
                } else {
                    x
                }
            }).collect::<Vec<_>>().into_boxed_slice())
        } else {
            Self::Unicode(value.to_string())
        }
    }
}

impl<'a> From<PackedString> for String {
    fn from(value: PackedString) -> Self {
        match value {
            PackedString::LowerLatinUnderscoreHyphenColonFiveBit(bytes) => {
                todo!()
            }
            PackedString::CasedLatinUnderscoreHyphenColonSixBit(_) => todo!(),
            PackedString::Unicode(s) => s,
            PackedString::Ascii(s) => s.into_iter().map(|x| (x ^ 0b1000_0000) as char).collect::<String>(),
        }
    }
}

impl OsmLiteralSerializable for PackedString {
    type Category = LiteralValue;

    fn serialize_to_pool(&self, _pool: &mut LiteralPool<LiteralValue>) -> Vec<u8> {
        match self {
            PackedString::LowerLatinUnderscoreHyphenColonFiveBit(b) => b.iter().copied().collect(),
            PackedString::CasedLatinUnderscoreHyphenColonSixBit(b) => b.iter().copied().collect(),
            PackedString::Ascii(b) => b.iter().copied().collect(),
            PackedString::Unicode(b) => b.bytes().collect(),
        }
    }
}
