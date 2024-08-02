pub mod bbox;
mod compare_by;
pub mod point_range;
mod tree;
pub mod tree_traits;
mod structure;
mod tree_serde;


#[cfg(test)]
mod test;

const NODE_SATURATION_POINT: usize = 8000;

use structure::LongLatTree;
pub use tree::*;
use tree_traits::{MultidimensionalKey, MultidimensionalValue};

use minimal_storage::serialize_min::SerializeMinimal;

pub fn open_tree<const D: usize, Key, Value>(
    state_path: std::path::PathBuf,
    global_area: Key::Parent,
) -> LongLatTree<D, Key, Value>
where
    Key: MultidimensionalKey<D>,
    Value: MultidimensionalValue<Key>,
    for<'serialize> <Value as SerializeMinimal>::ExternalData<'serialize>: Copy,
{
    std::fs::create_dir_all(&state_path).unwrap();

    let root_file = state_path.join("root");

    let geo_dir_rc = std::rc::Rc::new(state_path.clone());

    LongLatTree::new(global_area, state_path)
}
