use minimal_storage::serialize_min::SerializeMinimal;
use osm_literals::{literal::Literal, literal_value::LiteralValue, pool::LiteralPool};
use osmpbfreader::{OsmId, Way, WayId};

use crate::compressor::{compressed_data::flattened_id, tag_compressing::{self, way::inline_way_tags, InlinedTags}, CACHE_SATURATION};

use tree::{bbox::BoundingBox, point_range::StoredBinaryTree};

use super::CompressedOsmData;

pub fn osm_way_to_compressed_node(way: Way, bbox_cache: &mut StoredBinaryTree<{ CACHE_SATURATION }, u64, BoundingBox<i32>>) -> CompressedOsmData {

    let mut real_children = Vec::new();

    let bbox: BoundingBox<i32> = way.nodes.iter().filter_map(|child| {
        let id = flattened_id(&OsmId::Node(*child));
        let child_box = bbox_cache.find_first_item_at_key_exact(&id);

        let child_box = match child_box {
            Some(c) => c,
            None => {
                eprintln!("Node {child:?} (flattened ID: {id}) should exist. Encountered while building bounding box for way {:?}", way.id);
                return None;
        }
    };

        let child_box = child_box.into_inner();

        real_children.push(id);

        Some((*child_box.x(), *child_box.y()))
    }).collect();

    let tags = inline_way_tags(way.tags);


    CompressedOsmData::Way { bbox, tags, id: way.id, children: real_children, }
}

pub fn serialize_way<W: std::io::Write>(
    write_to: &mut W,
    pools: &mut (LiteralPool<Literal>, LiteralPool<LiteralValue>),
    id: &WayId,
    tags: &InlinedTags<tag_compressing::way::Way>,
    children: &Vec<u64>
) -> Result<(), std::io::Error> {
    
    //first byte layout:
    //0: not a node
    //1: yes a way
    //others: todo
    let header = 0b01_00_0000u8;

    write_to.write_all(&[header])?;

    id.0.minimally_serialize(write_to, ())?;

    //and just chuck all the literals into the literal pool and then put em at the end.
    let literals = &tags.other;

    literals.len().minimally_serialize(write_to, ())?;
    for literal in literals.iter() {
        let id = LiteralPool::<Literal>::insert(pools, literal)?;

        id.minimally_serialize(write_to, ())?;
    }

    //same with the nodeIds
    children.len().minimally_serialize(write_to, ())?;
    for child in children.iter() {
        child.minimally_serialize(write_to, ())?;
    }
    

    Ok(())
}