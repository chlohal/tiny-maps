use std::env;

use offline_tiny_maps::{compressor::Compressor, tree::{bbox::EARTH_BBOX, point_range::{Point, PointRange}}};

fn main() {
    let mut compressor = Compressor::new(env::current_dir().unwrap().join("Output"));

    for item in compressor.geography.deref().find_entries_in_box(&EARTH_BBOX) {
        dbg!(item);
    }

    for (id, item) in compressor.get_elements_bbox_in_range(&PointRange(0, u64::MAX)) {
        dbg!((id, item));
    }

    dbg!(compressor.get_element_bbox(&osmpbfreader::OsmId::Node(osmpbfreader::NodeId(8569371426))));
}