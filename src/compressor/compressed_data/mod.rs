use node::{osm_node_to_compressed_node, serialize_node};
use osmpbfreader::{NodeId, OsmId, OsmObj, RelationId, WayId};
use relation::osm_relation_to_compressed_node;
use way::osm_way_to_compressed_node;

use tree::{bbox::BoundingBox, point_range::{Point, StoredBinaryTree}, StoredPointTree};

use minimal_storage::{serialize_min::{DeserializeFromMinimal, ReadExtReadOne, SerializeMinimal}, varint::{from_varint, to_varint, ToVarint}};

use osm_literals::{literal_value::LiteralValue, literal::Literal, pool::LiteralPool};

use super::tag_compressing::{node::Node, InlinedTags};

mod node;
mod way;
mod relation;


#[derive(Clone, Debug)]
pub enum CompressedOsmData {
    Node{ id: NodeId, tags: InlinedTags<Node>, point: BoundingBox<i32> },
    Way{ bbox: BoundingBox<i32>, id: WayId },
    Relation{ bbox: BoundingBox<i32>, id: RelationId }
}

impl CompressedOsmData {
    pub fn bbox(&self) -> &BoundingBox<i32> {
        match self {
            CompressedOsmData::Node { point, .. } => point,
            CompressedOsmData::Way{ bbox, .. } => bbox,
            CompressedOsmData::Relation{ bbox, .. } => bbox,
        }
    }

    pub fn osm_id(&self) -> OsmId {
        match self {
            CompressedOsmData::Node { id, .. } => OsmId::Node(*id),
            CompressedOsmData::Way { id, .. } => OsmId::Way(*id),
            CompressedOsmData::Relation { id, .. } => OsmId::Relation(*id),
        }
    }

    pub fn make_from_obj(value: OsmObj, bbox_cache: &mut StoredPointTree<1, Point<u64>, BoundingBox<i32>>) -> Result<Self, OsmObj> {
        let value = match value {
            OsmObj::Node(n) => osm_node_to_compressed_node(n),
            OsmObj::Way(w) => osm_way_to_compressed_node(w, bbox_cache),
            OsmObj::Relation(r) => osm_relation_to_compressed_node(r, bbox_cache)?,
        };

        insert_bbox(&value.osm_id(), value.bbox().clone(), bbox_cache);

        Ok(value)
    }
}

pub fn flattened_id(osm_id: &OsmId) -> u64 {
    let mut inner = osm_id.inner_id() as u64;

    if inner.leading_zeros() < 2 {
        panic!("Excessively big OSM id; no further bits for the enum variant")
    }

    inner <<= 2;

    inner |= match osm_id {
        OsmId::Node(_) => 0,
        OsmId::Way(_) => 1,
        OsmId::Relation(_) => 2,
    };

    inner
}

pub fn unflattened_id(id: u64) -> OsmId {
    let variant = id & 0b11;

    let id = (id >> 2) as i64;

    match variant {
        0 => OsmId::Node(NodeId(id)),
        1 => OsmId::Way(WayId(id)),
        2 => OsmId::Relation(RelationId(id)),
        _ => panic!("Bad variant!")
    }
}


fn insert_bbox(id: &OsmId, bbox: BoundingBox<i32>, bbox_cache: &mut StoredBinaryTree<u64, BoundingBox<i32>>) {
    let inner = flattened_id(id);

    bbox_cache.ref_mut().insert(&Point(inner), bbox.into());
}

impl DeserializeFromMinimal for CompressedOsmData {
    type ExternalData<'a> = &'a BoundingBox<i32>;

    

    fn deserialize_minimal<'a, 'd: 'a, R: std::io::Read>(from: &'a mut R, external_data: Self::ExternalData<'d>) -> Result<Self, std::io::Error> {
        let header = from.read_one()?;

        let is_node = (header & 0b1000_0000) != 0;

        todo!()
    }
}

impl SerializeMinimal for CompressedOsmData {
    type ExternalData<'a> = &'a mut (LiteralPool<Literal>, LiteralPool<LiteralValue>);

    fn minimally_serialize<'a, 's: 'a, W: std::io::Write>(&'a self, write_to: &mut W, external_data: Self::ExternalData<'s>) -> std::io::Result<()> {
        match self {
            CompressedOsmData::Node { id, tags, point } => serialize_node(write_to, external_data, id, tags, point),
            CompressedOsmData::Way{ .. } => Ok(()), //todo
            CompressedOsmData::Relation{ .. } => Ok(()), //todo
        }
    }
}

#[derive(Clone, Debug)]
pub struct UncompressedOsmData(Vec<u8>);

impl UncompressedOsmData {
    pub fn new(data: &CompressedOsmData, pool: &mut (LiteralPool<Literal>, LiteralPool<LiteralValue>)) -> Self {
        let mut blob = Vec::new();
        data.minimally_serialize(&mut blob, pool).unwrap();

        UncompressedOsmData(blob)
    }
}

impl SerializeMinimal for UncompressedOsmData {
    type ExternalData<'a> = ();

    fn minimally_serialize<'a, 's: 'a, W: std::io::Write>(&'a self, write_to: &mut W, _external_data: ()) -> std::io::Result<()> {
        (self.0.len() as usize).write_varint(write_to)?;

        write_to.write_all(&self.0)
    }
}

impl DeserializeFromMinimal for UncompressedOsmData {
    type ExternalData<'a> = &'a BoundingBox<i32>;
    
    fn deserialize_minimal<'a, 'd: 'a, R: std::io::Read>(from: &'a mut R, _external_data: &'a BoundingBox<i32>) -> Result<Self, std::io::Error> {
        let len = from_varint::<usize>(from)?;

        let mut vec = vec![0; len];

        from.read_exact(&mut vec[0..len])?;

        Ok(Self(vec))
    }
}