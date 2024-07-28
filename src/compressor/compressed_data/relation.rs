use osmpbfreader::Relation;

use crate::{compressor::compressed_data::flattened_id, tree::{bbox::BoundingBox, point_range::{PointRange, StoredBinaryTree}}};

use super::CompressedOsmData;

pub fn osm_relation_to_compressed_node(relation: Relation, bbox_cache: &mut StoredBinaryTree<u64, BoundingBox<i32>>) -> CompressedOsmData {
    
    let bbox: BoundingBox<i32> = relation.refs.iter().map(|child| {
        let id = flattened_id(&child.member);
        
        let child_box = bbox_cache.deref().find_items_in_box(&PointRange(id, id)).next().unwrap().into_inner();

        child_box
    }).collect();


    CompressedOsmData::Relation { bbox, id: relation.id }
}