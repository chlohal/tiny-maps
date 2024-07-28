use minimal_storage::serialize_min::SerializeMinimal;
use crate::{literal::Literal, literal_value::LiteralValue, pool::LiteralPool};

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