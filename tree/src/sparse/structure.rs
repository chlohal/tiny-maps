use std::{
    collections::btree_map::Values,
    fmt::Debug,
    marker::PhantomData,
    sync::{
        atomic::{AtomicBool, AtomicUsize}, Arc, OnceLock
    },
};

use btree_vec::BTreeVec;
use minimal_storage::{
    multitype_paged_storage::{SingleTypeView, StoragePage, StoreByPage},
    paged_storage::{Page, PageId, PagedStorage}, pooled_storage::Filelike,
};

use crate::PAGE_SIZE;

use super::{SparseKey, SparseValue};

pub struct StoredTree<
    const DIMENSION_COUNT: usize,
    const NODE_SATURATION_POINT: usize,
    Key,
    Value,
    RootPage,
    Storage,
> where
    Key: SparseKey<DIMENSION_COUNT>,
    Value: SparseValue,
    Storage: StoreByPage<Inner<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>>,
    RootPage: StoragePage<Root<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value, Storage::PageId>>
{
    pub(crate) storage: Storage,
    pub(crate) root: Arc<RootPage>,
    pub(super) _sb: PhantomData<(Key, Value)>
}

pub type TreePagedStorage<
    const DIMENSION_COUNT: usize,
    const NODE_SATURATION_POINT: usize,
    Key,
    Value,
> = PagedStorage<{ PAGE_SIZE }, Inner<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>>;

pub struct Root<const DIMENSION_COUNT: usize, const NODE_SATURATION_POINT: usize, Key, Value, PageId>
where
    Key: SparseKey<DIMENSION_COUNT>,
    Value: SparseValue,
{
    pub(crate) root_bbox: Key::Parent,
    pub(crate) node: Node<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value, PageId>,
}

impl<const DIMENSION_COUNT: usize, const NODE_SATURATION_POINT: usize, Key, Value, PageId> Debug
    for Root<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value, PageId>
where
    Key: SparseKey<DIMENSION_COUNT> + Debug,
    Value: SparseValue,
    Key::Parent: Debug,
    PageId: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Root")
            .field("root_bbox", &self.root_bbox)
            .field("node", &self.node)
            .finish()
    }
}

pub(crate) struct Node<const DIMENSION_COUNT: usize, const NODE_SATURATION_POINT: usize, Key, Value, PageId>
where
    Key: SparseKey<DIMENSION_COUNT>,
    Value: SparseValue,
{
    pub(super) page_id: PageId,
    pub(super) bbox: Key::Parent,
    pub(super) child_count: AtomicUsize,
    pub(super) left_right_split: OnceLock<(
        Box<Node<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value, PageId>>,
        Box<Node<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value, PageId>>,
    )>,
    pub(super) __phantom: PhantomData<Value>,
}

impl<const DIMENSION_COUNT: usize, const NODE_SATURATION_POINT: usize, Key, Value, PageId> Debug
    for Node<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value, PageId>
where
    Key: SparseKey<DIMENSION_COUNT> + Debug,
    Value: SparseValue,
    Key::Parent: Debug,
    PageId: Debug
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Node")
            .field("page_id", &self.page_id)
            .field("bbox", &self.bbox)
            .field("left_right_split", &self.left_right_split)
            .field("child_count", &self.child_count)
            .finish()
    }
}

pub struct Inner<const DIMENSION_COUNT: usize, const NODE_SATURATION_POINT: usize, Key, Value>
where
    Key: SparseKey<DIMENSION_COUNT>,
    Value: SparseValue,
{
    pub(crate) children: BTreeVec<Key, Value>,
}
