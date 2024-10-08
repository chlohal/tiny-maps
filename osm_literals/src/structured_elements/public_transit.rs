use minimal_storage::serialize_min::SerializeMinimal;
use crate::{literal::Literal, literal_value::LiteralValue, pool::LiteralPool};

pub struct OsmPublicTransit;

impl SerializeMinimal for OsmPublicTransit {
    type ExternalData<'a> = &'a mut (LiteralPool<Literal>, LiteralPool<LiteralValue>);

    fn minimally_serialize<'a, 's: 'a, W: std::io::Write>(&'a self, _write_to: &mut W, _external_data: Self::ExternalData<'s>) -> std::io::Result<()> {
        todo!()
    }
}

impl From<OsmPublicTransit> for Literal {
    fn from(_value: OsmPublicTransit) -> Self {
        todo!()
    }
}