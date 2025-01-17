use std::io::Write;

use minimal_storage::{bit_sections::{BitSection, Byte}, pooled_storage::Pool, serialize_min::SerializeMinimal, varint::ToVarint};
use osm_tags_to_fields::fields::AnyOsmField;
use osm_value_atom::LiteralValue;

use osm_structures::structured_elements::{address::OsmAddress, contact::OsmContactInfo};

#[derive(Clone, Debug)]
pub enum Field {
    Other(LiteralValue, LiteralValue),
    Field(AnyOsmField),
}

impl<A: AsRef<str>, B: Into<LiteralValue>> From<(A, B)> for Field {
    fn from(value: (A, B)) -> Self {
        Field::Other(value.0.as_ref().into(), value.1.into())
    }
}

impl SerializeMinimal for Field {
    type ExternalData<'a> = &'a Pool<LiteralValue>;

    fn minimally_serialize<'a, 's: 'a, W: Write>(
        &'a self,
        write_to: &mut W,
        pool: Self::ExternalData<'s>,
    ) -> std::io::Result<()> {
        match self {
            Field::Other(k, v) => {
                let mut head = Byte::from(0);
                head.set_bit(0, false);
                write_to.write_all(&[head.into_inner()])?;

                let id = pool.insert(&k.clone().into(), ())?;
                id.write_varint(write_to)?;

                let id = pool.insert(v, ())?;
                return id.write_varint(write_to);
            }
            Field::Field(wk) => {
                let mut head = BitSection::<0, 16, u16>::from(0);
                head.set_bit(0, true);

                const {
                    assert!(osm_tags_to_fields::fields::MAX_FIELD_ID < 2usize.pow(10));
                }

                let head = head.reduce_extent::<1, 16>();

                return wk.minimally_serialize(write_to, (pool, head))
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
