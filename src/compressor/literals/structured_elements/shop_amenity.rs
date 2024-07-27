use crate::{compressor::literals::{literal_value::LiteralValue, Literal, LiteralPool}, storage::serialize_min::SerializeMinimal};

pub struct OsmShopAmenity;


impl SerializeMinimal for OsmShopAmenity {
    type ExternalData<'a> = &'a mut (LiteralPool<Literal>, LiteralPool<LiteralValue>);

    fn minimally_serialize<'a, 's: 'a, W: std::io::Write>(&'a self, write_to: &mut W, external_data: Self::ExternalData<'s>) -> std::io::Result<()> {
        todo!()
    }
}

impl From<OsmShopAmenity> for Literal {
    fn from(value: OsmShopAmenity) -> Self {
        todo!()
    }
}