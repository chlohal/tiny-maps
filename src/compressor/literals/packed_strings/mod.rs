pub enum PackedString {
    LowerLatinUnderscoreHyphenColonFiveBit(Box<[u8]>),
    CasedLatinUnderscoreHyphenColonSixBit(Box<[u8]>),
    Ascii(String),
    Unicode(String),
}

impl<'a> From<PackedString> for String {
    fn from(value: PackedString) -> Self {
        match value {
            PackedString::LowerLatinUnderscoreHyphenColonFiveBit(bytes) => {
                todo!()
            }
            PackedString::CasedLatinUnderscoreHyphenColonSixBit(_) => todo!(),
            PackedString::Unicode(s) => s,
            PackedString::Ascii(s) => s,
        }
    }
}
