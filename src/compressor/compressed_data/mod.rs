use node::{osm_node_to_compressed_node, serialize_node};
use osmpbfreader::{NodeId, OsmObj};

use crate::{storage::serialize_min::{DeserializeFromMinimal, SerializeMinimal}, tree::bbox::BoundingBox};

use super::{inlining::{node::Node, InlinedTags}, literals::{literal_value::LiteralValue, Literal, LiteralPool}, varint::{from_varint, to_varint}};

mod node;


pub enum CompressedOsmData {
    Node{ id: NodeId, tags: InlinedTags<Node>, point: BoundingBox<i32> },
    Way(),
    Relation()
}

impl CompressedOsmData {
    pub fn bbox(&self) -> &BoundingBox<i32> {
        match self {
            CompressedOsmData::Node { id, tags, point } => point,
            CompressedOsmData::Way() => todo!(),
            CompressedOsmData::Relation() => todo!(),
        }
    }
}

impl DeserializeFromMinimal for CompressedOsmData {
    type ExternalData<'a> = &'a BoundingBox<i32>;

    

    fn deserialize_minimal<'a, 'd: 'a, R: std::io::Read>(from: &'a mut R, external_data: Self::ExternalData<'d>) -> Result<Self, std::io::Error> {
        todo!()
    }
}

impl SerializeMinimal for CompressedOsmData {
    type ExternalData<'a> = &'a mut (LiteralPool<Literal>, LiteralPool<LiteralValue>);

    fn minimally_serialize<'a, 's: 'a, W: std::io::Write>(&'a self, write_to: &mut W, external_data: Self::ExternalData<'s>) -> std::io::Result<()> {
        match self {
            CompressedOsmData::Node { id, tags, point } => serialize_node(write_to, external_data, id, tags, point),
            CompressedOsmData::Way() => todo!(),
            CompressedOsmData::Relation() => todo!(),
        }
    }
}

impl From<OsmObj> for CompressedOsmData {
    fn from(value: OsmObj) -> Self {
        match value {
            OsmObj::Node(n) => osm_node_to_compressed_node(n),
            OsmObj::Way(_) => todo!(),
            OsmObj::Relation(_) => todo!(),
        }
    }
}

pub struct UncompressedOsmData(Vec<u8>);

impl UncompressedOsmData {
    pub fn new(data: CompressedOsmData, pool: &mut (LiteralPool<Literal>, LiteralPool<LiteralValue>)) -> Self {
        let mut blob = Vec::new();
        data.minimally_serialize(&mut blob, pool).unwrap();

        UncompressedOsmData(blob)
    }
}

impl SerializeMinimal for UncompressedOsmData {
    type ExternalData<'a> = ();

    fn minimally_serialize<'a, 's: 'a, W: std::io::Write>(&'a self, write_to: &mut W, external_data: Self::ExternalData<'s>) -> std::io::Result<()> {
        write_to.write_all(&to_varint::<usize>(self.0.len()))?;

        write_to.write_all(&self.0)
    }
}

impl DeserializeFromMinimal for UncompressedOsmData {
    type ExternalData<'a> = &'a BoundingBox<i32>;
    
    fn deserialize_minimal<'a, 'd: 'a, R: std::io::Read>(from: &'a mut R, external_data: Self::ExternalData<'d>) -> Result<Self, std::io::Error> {
        let len = from_varint::<usize>(from)?;

        let mut vec = Vec::with_capacity(len);

        from.read_exact(&mut vec[0..len])?;

        Ok(Self(vec))
    }
}