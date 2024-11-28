use std::io::Write;

use minimal_storage::{bit_sections::Byte, pooled_storage::Pool, serialize_min::SerializeMinimal, varint::ToVarint};
use osm_value_atom::LiteralValue;

use osm_structures::structured_elements::{address::OsmAddress, contact::OsmContactInfo};

#[derive(Clone, Debug)]
pub enum Field {
    Other(LiteralKey, LiteralValue),
    Field(AnyOsmField),
}

impl<A: Into<LiteralKey>, B: Into<LiteralValue>> From<(A, B)> for Field {
    fn from(value: (A, B)) -> Self {
        Field::Other(value.0.into(), value.1.into())
    }
}

impl SerializeMinimal for Field {
    type ExternalData<'a> = &'a mut Pool<LiteralValue>;

    fn minimally_serialize<'a, 's: 'a, W: Write>(
        &'a self,
        write_to: &mut W,
        pool: Self::ExternalData<'s>,
    ) -> std::io::Result<()> {
        let mut head = Byte::from(0);

        match self {
            Field::KeyVar(k, v) => {
                head.set_bit(0, false);

                match k {
                    LiteralKey::WellKnownKey(wkk) => {

                        head.set_range::<1, 8>(*wkk as u8);
                        write_to.write_all(&[head.into_inner()])?;
                    }
                    LiteralKey::Str(s) => {
                        head.set_bit(1, true);
                        write_to.write_all(&[head.into_inner()])?;

                        let id = pool.insert(&s.clone().into(), ())?;
                        id.write_varint(write_to)?;
                    }
                }

                let id = pool.insert(v, ())?;
                return id.write_varint(write_to);
            }
            Field::WellKnownKeyVar(wk) => {
                head.set_bit(0, true);

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
