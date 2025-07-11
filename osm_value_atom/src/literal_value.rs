use std::io::Read;

use minimal_storage::{serialize_min::{ DeserializeFromMinimal, MinimalSerializedSeek, ReadExtReadOne, SerializeMinimal }, varint::{ from_varint, ToVarint } };


#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LiteralValue {
    Specificvalue(LiteralValueSpecificValue), //0
    UInt(u64), //1
    IInt(i64), //2
    TinyUNumber(u8), //3
    TinyINumber(i8), //4
    Date(usize, usize, usize), //5
    Time(usize, usize), //6
    ListWithSep(u8, Vec<LiteralValue>), //7
    TwoUpperLatinAbbrev(u8, u8), //8
    SplitSemiList(Vec<LiteralValue>), //9

    String(String), //10,11,12 NOTE: THIS TAKES UP THE EQUIVALENT OF 4 VARIANTS IN THE BINARY REPRESENTATION

    Ref(u64), //13
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LiteralValueSpecificValue {
    BoolYes,
    BoolNo,
    Blank,
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
}

impl<T: AsRef<str>> From<T> for LiteralValue {
    fn from(value: T) -> Self {
        let value = value.as_ref();

        match value {
            "" => return LiteralValue::Specificvalue(LiteralValueSpecificValue::Blank),
            "yes" => return LiteralValue::Specificvalue(LiteralValueSpecificValue::BoolYes),
            "no" => return LiteralValue::Specificvalue(LiteralValueSpecificValue::BoolNo),
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

    

    fn deserialize_minimal<'a, 'd: 'a, R: Read>(from: &'a mut R, _external_data: ()) -> Result<Self, std::io::Error> {
        let header_byte = from.read_one()?;

        let enum_variant = header_byte >> 4;
        let lower_nibble_ext_info = header_byte & 0b1111;

        let res = match enum_variant {
            0b0000 => LiteralValue::Specificvalue({
                match lower_nibble_ext_info {
                    0b0000 => LiteralValueSpecificValue::Blank,
                    0b0001 => LiteralValueSpecificValue::BoolNo,
                    0b0010 => LiteralValueSpecificValue::BoolYes,
                    _ => unreachable!()
                }
            }),
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
                LiteralValue::String(String::deserialize_minimal(from, Some(header_byte))?)
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
            LiteralValue::Specificvalue(v) => 0b0000 & {
                match v {
                    LiteralValueSpecificValue::BoolYes => 0b10,
                    LiteralValueSpecificValue::BoolNo => 0b01,
                    LiteralValueSpecificValue::Blank => 0b00,
                }
            },
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
                //the string's serializer handles the process of writing the header byte.
                //this prevents excessive buffer usage
                return s.as_str().minimally_serialize(&mut buf,  0b1100_0000u8.into());
            },

            LiteralValue::Ref(_) => panic!("Unable to serialize a reference!"),
        };

        buf[0] = header_byte;
        
        write_to.write_all(&buf)
    }
}

impl MinimalSerializedSeek for LiteralValue {
    fn seek_past<R: Read>(from: &mut R) -> std::io::Result<()> {
        //todo: make this more optimal wrt seeking
        Self::deserialize_minimal(from, ()).map(|_| ())
    }
}