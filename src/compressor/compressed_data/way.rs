use osmpbfreader::{OsmId, Way};

use crate::compressor::compressed_data::flattened_id;

use tree::{bbox::BoundingBox, point_range::StoredBinaryTree};

use super::CompressedOsmData;

pub fn osm_way_to_compressed_node(way: Way, bbox_cache: &mut StoredBinaryTree<u64, BoundingBox<i32>>) -> CompressedOsmData {
    
    let bbox: BoundingBox<i32> = way.nodes.iter().map(|child| {
        let id = flattened_id(&OsmId::Node(*child));
        
        let child_box = bbox_cache.find_first_item_at_key_exact(&id).unwrap().into_inner();

        (*child_box.x(), *child_box.y())
    }).collect();


    CompressedOsmData::Way { bbox, id: way.id }
}