use crate::{compressor::literals::{literal_value::LiteralValue, Literal, LiteralPool}, storage::serialize_min::SerializeMinimal};

pub struct OsmPublicTransit;

impl SerializeMinimal for OsmPublicTransit {
    type ExternalData<'a> = &'a mut (LiteralPool<Literal>, LiteralPool<LiteralValue>);

    fn minimally_serialize<'a, 's: 'a, W: std::io::Write>(&'a self, write_to: &mut W, external_data: Self::ExternalData<'s>) -> std::io::Result<()> {
        todo!()
    }
}

impl From<OsmPublicTransit> for Literal {
    fn from(value: OsmPublicTransit) -> Self {
        todo!()
    }
}