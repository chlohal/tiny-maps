use osm_tag_compression::compressed_data::flattened_id;
use tree::{bbox::BoundingBox, open_tree_dense, open_tree_sparse, point_range::DisregardWhenDeserializing};




fn main() {
    tree()
}

fn tree() {
    let tree = open_tree_sparse::<1, 16_000, u64, BoundingBox<i32>>(
        std::env::current_dir().unwrap().join(".map/tmp.bboxes"),
        0..=u64::MAX,
    );

    dbg!(tree.find_entries_in_box(&(0..=u64::MAX)).collect::<Vec<_>>());

    dbg!(tree.find_first_item_at_key_exact(&flattened_id(&osmpbfreader::OsmId::Node(osmpbfreader::NodeId(1675160938)))));
}
