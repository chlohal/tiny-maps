use std::io::Write;

use minimal_storage::{serialize_min::SerializeMinimal, varint::ToVarint};

use crate::{literal_value::LiteralValue, pool::LiteralPool, structured_elements::{address::OsmAddress, contact::OsmContactInfo}};

#[derive(Clone, Debug)]
pub enum Literal {
    KeyVar(LiteralKey, LiteralValue),
    WellKnownKeyVar(WellKnownKeyVar),

    Ref(u64),
}

impl<A: Into<LiteralKey>, B: Into<LiteralValue>> From<(A, B)> for Literal {
    fn from(value: (A, B)) -> Self {
        Literal::KeyVar(value.0.into(), value.1.into())
    }
}


impl SerializeMinimal for Literal {
    type ExternalData<'a> = &'a mut (LiteralPool<Literal>, LiteralPool<LiteralValue>);

    fn minimally_serialize<'a, 's: 'a, W: Write>(
        &'a self,
        write_to: &mut W,
        pool: Self::ExternalData<'s>,
    ) -> std::io::Result<()> {
        let mut head = 0b0000_0000u8;

        match self {
            Literal::KeyVar(k, v) => {
                head |= 0b0 << 7;

                match k {
                    LiteralKey::WellKnownKey(wkk) => {
                        debug_assert!((*wkk as u8) & 0b1100_0000 == 0);

                        head |= *wkk as u8;
                        write_to.write_all(&[head])?;
                    }
                    LiteralKey::Str(s) => {
                        head |= 1 << 6;
                        write_to.write_all(&[head])?;

                        let id = pool.1.insert(&s.clone().into())?;
                        id.write_varint(write_to)?;
                    }
                }

                let id = pool.1.insert(v)?;
                return id.write_varint(write_to);
            }
            Literal::WellKnownKeyVar(wk) => {
                head |= 0b1 << 7;
                match wk {
                    WellKnownKeyVar::Address(addr) => {
                        head |= 0b00_0000;

                        write_to.write_all(&[head])?;

                        return addr.minimally_serialize(write_to, pool);
                    }
                    WellKnownKeyVar::Contact(contact) => {
                        head |= 0b00_0001;

                        write_to.write_all(&[head])?;

                        return contact.minimally_serialize(write_to, pool);
                    }
                    WellKnownKeyVar::MapFeatureType => {
                        head |= 0b00_0010;

                        write_to.write_all(&[head])?;

                        todo!()
                    }
                }
            }
            Literal::Ref(_) => panic!("Unable to serialize a reference!"),
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub enum LiteralKey {
    WellKnownKey(WellKnownKey),
    Str(String),
}

impl<R: AsRef<str>> From<R> for LiteralKey {
    fn from(value: R) -> Self {
        Self::Str(value.as_ref().to_string())
    }
}

#[derive(Clone, Debug)]
pub enum WellKnownKeyVar {
    Address(OsmAddress),
    MapFeatureType,
    Contact(OsmContactInfo),
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[repr(u8)]
pub enum WellKnownKey {
    Waterway = 0,
}
