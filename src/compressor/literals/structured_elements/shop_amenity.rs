use crate::compressor::literals::{Literal, OsmLiteralSerializable};

pub struct OsmShopAmenity;

impl OsmLiteralSerializable for OsmShopAmenity {
    type Category = Literal;

    fn serialize_to_pool(
        &self,
        pool: &mut <Self::Category as crate::compressor::literals::OsmLiteralArchetype>::SerializationReference,
    ) -> Vec<u8> {
        todo!()
    }
}