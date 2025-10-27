pub mod bbox;
mod compare_by;
pub mod point_range;
pub mod tree_traits;

pub mod dense;
pub mod sparse;

#[cfg(test)]
mod test;

pub const PAGE_SIZE: usize = 8;

use minimal_storage::{multitype_paged_storage::{SingleTypeView, StoreByPage}, paged_storage::PageId};
use sparse::{SparseKey, SparseValue};
use tree_traits::{MultidimensionalKey, MultidimensionalValue};

use crate::sparse::structure::Inner;

pub fn open_tree_dense<const D: usize, const S: usize, Key, Value>(
    state_path: std::path::PathBuf,
    global_area: Key::Parent,
) -> dense::structure::StoredTree<D, S, Key, Value>
where
    Key: MultidimensionalKey<D>,
    Value: MultidimensionalValue<Key>,
{
    std::fs::create_dir_all(&state_path).unwrap();

    dense::structure::StoredTree::new(global_area, state_path)
}

pub fn open_tree_sparse<const D: usize, const S: usize, Key, Value>(
    state_path: std::path::PathBuf,
    global_area: Key::Parent,
) -> sparse::structure::StoredTree<
    D,
    S,
    Key,
    Value,
    minimal_storage::paged_storage::Page<
        PAGE_SIZE,
        sparse::structure::Root<D, S, Key, Value>,
        std::fs::File,
    >,
    SingleTypeView<PAGE_SIZE, std::fs::File, Inner<D, S, Key, Value>>,
>
where
    Key: SparseKey<D>,
    Value: SparseValue,
{
    std::fs::create_dir_all(&state_path).unwrap();

    sparse::open_file(global_area, state_path)
}
