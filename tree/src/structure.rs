use std::marker::PhantomData;

use btree_vec::BTreeVec;
use minimal_storage::
    paged_storage::{PageId, PagedStorage}
;

use crate::{tree_traits::{MultidimensionalKey, MultidimensionalValue}, PAGE_SIZE};

pub struct StoredTree<const DIMENSION_COUNT: usize, const NODE_SATURATION_POINT: usize, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
{
    pub(crate) storage: TreePagedStorage<DIMENSION_COUNT,  NODE_SATURATION_POINT, Key, Value>,
    pub(crate) root: Root<DIMENSION_COUNT,  NODE_SATURATION_POINT, Key, Value>,
    pub(crate) structure_dirty: bool,
    pub(crate) structure_file: std::fs::File,
}

pub type TreePagedStorage<const DIMENSION_COUNT: usize, const NODE_SATURATION_POINT: usize, Key, Value> =
    PagedStorage<{PAGE_SIZE}, Inner<DIMENSION_COUNT,  NODE_SATURATION_POINT, Key, Value>>;

pub(crate) struct Root<const DIMENSION_COUNT: usize, const NODE_SATURATION_POINT: usize, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
{
    pub(crate) root_bbox: Key::Parent,
    pub(crate) node: Node<DIMENSION_COUNT,  NODE_SATURATION_POINT, Key, Value>,
}

pub(crate) struct Node<const DIMENSION_COUNT: usize, const NODE_SATURATION_POINT: usize, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
{
    pub(super) page_id: PageId<PAGE_SIZE>,
    pub(super) bbox: Key::Parent,
    pub(super) left_right_split: Option<(
        Box<Node<DIMENSION_COUNT,  NODE_SATURATION_POINT, Key, Value>>,
        Box<Node<DIMENSION_COUNT,  NODE_SATURATION_POINT, Key, Value>>,
    )>,
    pub(super) __phantom: PhantomData<Value>,
    pub(super) id: u64,
}

pub struct Inner<const DIMENSION_COUNT: usize, const NODE_SATURATION_POINT: usize, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
{
    pub(crate) children: BTreeVec<Key::DeltaFromParent, Value>,
}