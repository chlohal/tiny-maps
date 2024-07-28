use osmpbfreader::{OsmObj, Relation};

use crate::compressor::compressed_data::flattened_id;

use tree::{bbox::BoundingBox, point_range::{Point, StoredBinaryTree}};

use super::CompressedOsmData;

pub fn osm_relation_to_compressed_node(relation: Relation, bbox_cache: &mut StoredBinaryTree<u64, BoundingBox<i32>>) -> Result<CompressedOsmData, OsmObj> {

    let bbox: Option<BoundingBox<i32>> = relation.refs.iter().map(|child| {
        let id = flattened_id(&child.member);
        
        let child_box = bbox_cache.deref().find_first_item_at_key_exact(&Point(id))?.into_inner();

        Some(child_box)
    }).collect();

    let Some(bbox) = bbox else { return Err(OsmObj::Relation(relation)) };


    Ok(CompressedOsmData::Relation { bbox, id: relation.id })
}