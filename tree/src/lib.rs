pub mod bbox;
mod compare_by;
pub mod point_range;
mod tree;
pub mod tree_traits;

mod tree_serde;

const NODE_SATURATION_POINT: usize = 8_000;

pub use tree::*;
use tree_traits::{MultidimensionalKey, MultidimensionalValue};

use minimal_storage::serialize_min::SerializeMinimal;

pub fn open_tree<const D: usize, Key, Value>(
    state_path: std::path::PathBuf,
    global_area: Key::Parent,
) -> StoredTree<D, Key, Value>
where
    Key: MultidimensionalKey<D>,
    Value: MultidimensionalValue<Key>,
    for<'serialize> <Value as SerializeMinimal>::ExternalData<'serialize>: Copy,
{
    std::fs::create_dir_all(&state_path).unwrap();

    let tree_structure_file = std::fs::File::options()
        .create(true)
        .write(true)
        .read(true)
        .open(state_path.join("structure"))
        .unwrap();

    let geo_dir_rc = std::rc::Rc::new((state_path.clone(), tree_structure_file));

    let root_file = state_path.join("root");

    if root_file.exists() {
        StoredTree::<D, Key, Value>::open(root_file, (geo_dir_rc, 1, global_area))
    } else {
        StoredTree::<D, Key, Value>::new(
            root_file,
            LongLatTree::<D, Key, Value>::new(global_area.clone(), std::rc::Rc::clone(&geo_dir_rc)),
            (geo_dir_rc, 1, global_area),
        )
    }
}
