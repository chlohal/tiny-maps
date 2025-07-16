use std::{
    fmt::Debug,
    marker::PhantomData,
    sync::{
        atomic::{AtomicBool, AtomicUsize},
        Mutex, OnceLock, RwLock,
    },
};

use btree_vec::BTreeVec;
use minimal_storage::paged_storage::{PageId, PagedStorage};

use crate::{
    tree_traits::{MultidimensionalKey, MultidimensionalValue},
    PAGE_SIZE,
};

pub struct StoredTree<const DIMENSION_COUNT: usize, const NODE_SATURATION_POINT: usize, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
{
    pub(crate) storage: TreePagedStorage<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>,
    pub(crate) root: Root<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>,
    pub(crate) structure_dirty: AtomicBool,
    pub(crate) structure_file: std::fs::File,
}

impl<const D: usize, const N: usize, K, V> Debug for StoredTree<D, N, K, V>
where
    K: MultidimensionalKey<D>,
    V: MultidimensionalValue<K>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoredTree")
            .field("root", &self.root)
            .field("structure_dirty", &self.structure_dirty)
            .field("structure_file", &self.structure_file)
            .finish()
    }
}

pub type TreePagedStorage<
    const DIMENSION_COUNT: usize,
    const NODE_SATURATION_POINT: usize,
    Key,
    Value,
> = PagedStorage<{ PAGE_SIZE }, Inner<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>>;

#[derive(Debug)]
pub(crate) struct Root<const DIMENSION_COUNT: usize, const NODE_SATURATION_POINT: usize, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
{
    pub(crate) root_bbox: Key::Parent,
    pub(crate) node: Node<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>,
}

pub struct ExternalChildrenCount<
    const DIMENSION_COUNT: usize,
    const NODE_SATURATION_POINT: usize,
    Key,
    Value,
>(AtomicUsize, PhantomData<Key>, PhantomData<Value>)
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>;

impl<const DIMENSION_COUNT: usize, const NODE_SATURATION_POINT: usize, Key, Value> From<usize>
    for ExternalChildrenCount<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
{
    fn from(value: usize) -> Self {
        Self(value.into(), PhantomData, PhantomData)
    }
}

impl<const DIMENSION_COUNT: usize, const NODE_SATURATION_POINT: usize, Key, Value>
    ExternalChildrenCount<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
{
    ///Increments the current value, returning the previous value.
    /// Panics on overflow in non-optimized builds; otherwise overflows
    pub fn increment(
        &self,
        _inner: &mut Inner<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>,
    ) -> usize {
        let v = self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        debug_assert_ne!(v, usize::MAX);
        v
    }

    pub fn set(
        &self,
        _inner: &mut Inner<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>,
        val: usize,
    ) {
        self.0.store(val, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn get_initial(&self, _id: &PageId<{ PAGE_SIZE }>) -> usize {
        self.0.load(std::sync::atomic::Ordering::SeqCst)
    }
    pub fn get_maybe_initial(&self, id: &Option<PageId<{ PAGE_SIZE }>>) -> usize {
        if id.is_none() {
            return 0;
        } else {
            self.0.load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    pub fn get(&self, _inner: &Inner<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>) -> usize {
        self.0.load(std::sync::atomic::Ordering::SeqCst)
    }
}

pub(crate) struct Node<const DIMENSION_COUNT: usize, const NODE_SATURATION_POINT: usize, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
{
    pub(super) page_id: RwLock<Option<PageId<{ PAGE_SIZE }>>>,
    pub(super) children_count:
        ExternalChildrenCount<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>,
    pub(super) bbox: Key::Parent,
    pub(super) left_right_split: OnceLock<(
        Box<Node<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>>,
        Box<Node<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>>,
    )>,
    pub(super) __phantom: PhantomData<Value>,
    pub(super) id: u64,
}

impl<const DIMENSION_COUNT: usize, const NODE_SATURATION_POINT: usize, Key, Value> std::fmt::Debug
    for Node<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Node")
            .field("page_id", &self.page_id)
            .field("left_right_split", &self.left_right_split)
            .field("id", &self.id)
            .finish()
    }
}

#[derive(Debug)]
pub struct Inner<const DIMENSION_COUNT: usize, const NODE_SATURATION_POINT: usize, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
{
    pub(crate) children: BTreeVec<Key::DeltaFromParent, Value>,
}
