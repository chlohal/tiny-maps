use std::path::PathBuf;

use btree_vec::BTreeVec;
use minimal_storage::{serialize_min::SerializeMinimal, Storage};

use crate::tree_traits::{MultidimensionalKey, MultidimensionalValue};

pub struct LongLatTree<const DIMENSION_COUNT: usize, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
    for<'serialize> <Value as SerializeMinimal>::ExternalData<'serialize>: Copy,
{
    pub(crate) storage_folder: PathBuf,
    pub(crate) root: Root<DIMENSION_COUNT, Key, Value>,
    pub(crate) structure_file: std::fs::File,
    pub(crate) structure_dirty: bool,
}

pub(crate) struct Root<const DIMENSION_COUNT: usize, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
    for<'serialize> <Value as SerializeMinimal>::ExternalData<'serialize>: Copy,
{
    pub(crate) root_bbox: Key::Parent,
    pub(crate) node: Node<DIMENSION_COUNT, Key, Value>,
}

pub(crate) struct Node<const DIMENSION_COUNT: usize, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
    for<'serialize> <Value as SerializeMinimal>::ExternalData<'serialize>: Copy,
{
    pub(super) bbox: Key::Parent,
    pub(super) values: StoredChildren<DIMENSION_COUNT, Key, Value>,
    pub(super) left_right_split: Option<(
        Box<Node<DIMENSION_COUNT, Key, Value>>,
        Box<Node<DIMENSION_COUNT, Key, Value>>,
    )>,
    pub(super) id: u64,
}

pub struct Inner<const DIMENSION_COUNT: usize, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
    for<'serialize> <Value as SerializeMinimal>::ExternalData<'serialize>: Copy,
{
    pub(crate) children: BTreeVec<Key::DeltaFromParent, Value>,
}

pub(crate) type StoredChildren<const D: usize, K, T> =
    Storage<<K as MultidimensionalKey<D>>::Parent, Inner<D, K, T>>;
