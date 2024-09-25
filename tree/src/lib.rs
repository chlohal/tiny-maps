pub mod bbox;
mod compare_by;
pub mod point_range;
mod tree;
pub mod tree_traits;
pub mod structure;
mod tree_serde;


#[cfg(test)]
mod test;

pub const PAGE_SIZE: usize = 8;

const NODE_SATURATION_POINT: usize = 8000;

use structure::LongLatTree;
pub use tree::*;
use tree_traits::{MultidimensionalKey, MultidimensionalValue};

pub fn open_tree<const D: usize, Key, Value>(
    state_path: std::path::PathBuf,
    global_area: Key::Parent,
) -> LongLatTree<D, Key, Value>
where
    Key: MultidimensionalKey<D>,
    Value: MultidimensionalValue<Key>,
{
    std::fs::create_dir_all(&state_path).unwrap();

    LongLatTree::new(global_area, state_path)
}
