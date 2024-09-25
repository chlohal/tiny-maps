use minimal_storage::paged_storage::{PageId, PagedStorage};
use tree::{bbox::BoundingBox, open_tree, point_range::DisregardWhenDeserializing};

use tree::structure::TreePagedStorage;


fn main() {
    tree()
}

fn tree() {
    let mut tree = open_tree::<1, u64, DisregardWhenDeserializing<u64, BoundingBox<u32>>>(
        std::env::current_dir().unwrap().join(".map/tmp.bboxes"),
        0..=u64::MAX,
    );

    dbg!(tree.find_first_item_at_key_exact(&24418126920));
}
