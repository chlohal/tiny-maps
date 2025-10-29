use std::{
    convert::Infallible,
    fs::File,
    io::{BufWriter, Seek, Write},
    marker::PhantomData,
    ops::Deref,
    path::PathBuf,
    sync::Arc,
};

use btree_vec::{BTreeVec, SeparateStateIteratable};
use debug_logs::debug_print;

use crate::{
    sparse::structure::{Inner, Node, Root, StoredTree, TreePagedStorage},
    PAGE_SIZE,
};

use crate::tree_traits::{Dimension, MultidimensionalParent};
use minimal_storage::{
    multitype_paged_storage::{MultitypePagedStorage, StoragePage, StoreByPage},
    paged_storage::{Page, PageId},
};

use super::{SparseKey, SparseValue};

impl<
        const DIMENSION_COUNT: usize,
        const NODE_SATURATION_POINT: usize,
        Key,
        Value,
        RootPage,
        Storage,
    > StoredTree<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value, RootPage, Storage>
where
    Key: SparseKey<DIMENSION_COUNT>,
    Value: SparseValue,
    Storage: StoreByPage<
        Inner<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>,
        PageId = PageId<PAGE_SIZE>,
    >,
    RootPage: StoragePage<Root<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>>,
{
    pub fn flush<'s>(&'s mut self) -> std::io::Result<()> {
        self.storage.flush();
        Ok(())
    }
    pub fn find_items_in_box<'a>(
        &'a self,
        query: &'a Key::Parent,
    ) -> impl Iterator<Item = Value> + 'a {
        self.find_entries_in_box(query).map(|x| x.1)
    }

    pub fn find_entries_in_box<'a, 's: 'a>(
        &'s self,
        query: &'a Key::Parent,
    ) -> impl Iterator<Item = (Key, Value)> + 'a {
        #[allow(dead_code)]
        struct FindEntriesInBox<RAII, I> {
            //safety: the fields MUST be in this order to prevent UB in
            // the instant that the RAII guard is freed before the inner
            // field.
            inner_iter: I,
            root_page: RAII,
        }
        impl<RAII, I: Iterator> Iterator for FindEntriesInBox<RAII, I> {
            type Item = I::Item;

            fn next(&mut self) -> Option<Self::Item> {
                self.inner_iter.next()
            }
        }
        let root_page = StoragePage::read_arc(&self.root);

        //safety: the RAII guard is stored into the FindEntriesInBox struct, which is in charge of keeping the
        // guard alive and therefore the validity of the pointer.
        let inner_iter = unsafe {
            self.root
                .as_ptr()
                .as_ref()
                .unwrap()
                .search_all_nodes_touching_area(query)
        };

        (FindEntriesInBox {
            inner_iter,
            root_page,
        })
        .filter(|(_, bbox)| bbox.overlaps(query))
        .flat_map(move |(node, _bbox)| {
            let page_read = Storage::Page::read_arc(&self.storage.get(&node.page_id, ()).unwrap());

            let mut iter_state = page_read.children.begin_range(Key::smallest_key_in(query));

            std::iter::from_fn(move || loop {
                let (k, v) = page_read.children.stateless_next(&mut iter_state)?;
                return Some((k.to_owned(), v.to_owned()));
            })
            .flat_map(|(k, vs)| vs.into_iter().map(move |v| (k.clone(), v)))
        })
    }

    pub fn root_page_id(&self) -> PageId<PAGE_SIZE> {
        self.root_page_id
    }

    pub fn get<'a, 'b>(&'a self, query: &'b Key) -> Option<Value> {
        let root = self.root.read();
        let (leaf, _leaf_bbox) = root.search_leaf_for_key(query);

        let page: std::sync::Arc<<Storage as StoreByPage<_>>::Page> =
            self.storage.get(&leaf.page_id, ()).unwrap();

        let item = page.read().children.get(&query)?.iter().next().cloned();

        item
    }

    ///
    /// This behaves exactly like mapping the provided sorted `iter` over `get()`,
    /// with optimiziations to avoid re-fetching pages from physical storage.
    ///
    /// The iterator MUST be sorted as determined by the Ord trait for `Key` ; otherwise,
    /// the return is unspecified, but will not result in undefined behaviour.
    ///
    /// The returned iterator will NOT be sorted (except in certain trivial cases).
    ///
    /// An empty iterator will currently panic for implementation reasons.
    ///
    pub fn get_all<'a>(
        &'a self,
        mut iter: impl Iterator<Item = Key> + 'a,
    ) -> impl Iterator<Item = (Key, Value)> + 'a {
        return iter.filter_map(|x| {
            let value = self.get(&x)?;
            Some((x, value))
        });

        let mut query = iter.next().unwrap();
        let (Node { page_id, .. }, _leaf_bbox) = self.root.read().search_leaf_for_key(&query);

        let mut page_is_exact = true;
        let mut page = self.storage.get(&page_id, ()).unwrap();

        std::iter::from_fn(move || {
            loop {
                let page_lock = page.read();
                let children = &page_lock.children;

                let Some(column) = children.get(&query) else {
                    if page_is_exact {
                        //if it wasn't found using the exact correct page, then it wouldn't be anywhere else either.
                        query = iter.next()?;
                        page_is_exact = false;
                        continue;
                    } else {
                        //if it wasn't found, but the page was inexact, then get the exact page and try again.
                        drop(page_lock);
                        page_is_exact = true;
                        let rr = self.root.read();
                        let (Node { page_id, .. }, _leaf_bbox) = rr.search_leaf_for_key(&query);
                        page = self.storage.get(&page_id, ()).unwrap();
                        continue;
                    }
                };

                query = iter.next()?;
                page_is_exact = false;

                let value = column.front().to_owned();
                return Some((query.to_owned(), value));
            }
        });
    }

    pub fn insert(&self, k: Key, item: Value) {
        debug_print!("begin insert()");

        let root_read = self.root.read();
        let (leaf, structure_changed) =
            root_read.get_key_leaf_splitting_if_needed(&k, &self.storage);

        debug_print!("got leaf");

        let page = self.storage.get(&leaf.page_id, ()).unwrap();
        let mut child = page.write();

        debug_print!("got page");

        leaf.child_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        child.children.push(k, item);

        debug_print!("pushed");

        drop(child);
        drop(page);
    }

    pub fn expand_to_depth(&mut self, depth: usize) {
        let mut root = self.root.write();

        let Root {
            ref mut node,
            ref root_bbox,
        } = &mut *root;

        node.expand_to_depth(
            depth,
            &root_bbox,
            &mut self.storage,
            &Dimension::arbitrary_first(),
        )
    }
}

impl<const DIMENSION_COUNT: usize, const NODE_SATURATION_POINT: usize, Key, Value>
    Root<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>
where
    Key: SparseKey<DIMENSION_COUNT>,
    Value: SparseValue,
{
    fn search_all_nodes_touching_area<'a>(
        &'a self,
        area: &'a Key::Parent,
    ) -> impl Iterator<
        Item = (
            &Node<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>,
            Key::Parent,
        ),
    > + 'a {
        let mut search_stack = vec![self.search_smallest_node_covering_area(area)];

        std::iter::from_fn(move || {
            let (parent, bbox, direction) = search_stack.pop()?;

            match &parent.left_right_split.get() {
                Some((left, right)) => {
                    let (left_bbox_calculated, right_bbox_calculated) =
                        bbox.split_evenly_on_dimension(&direction);

                    if left_bbox_calculated.overlaps(&area) {
                        search_stack.push((left, left_bbox_calculated, direction.next_axis()));
                    } else if right_bbox_calculated.overlaps(&area) {
                        search_stack.push((right, right_bbox_calculated, direction.next_axis()));
                    }
                }
                None => {}
            }

            return Some((parent, bbox));
        })
    }

    fn search_smallest_node_covering_area(
        &self,
        area: &Key::Parent,
    ) -> (
        &Node<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>,
        Key::Parent,
        <Key::Parent as MultidimensionalParent<DIMENSION_COUNT>>::DimensionEnum,
    ) {
        let mut tree = &self.node;
        let mut bbox = self.root_bbox.to_owned();
        let mut direction =
            <Key::Parent as MultidimensionalParent<DIMENSION_COUNT>>::DimensionEnum::arbitrary_first();

        loop {
            match &tree.left_right_split.get() {
                Some((left, right)) => {
                    let (left_bbox_calculated, right_bbox_calculated) =
                        bbox.split_evenly_on_dimension(&direction);

                    if left_bbox_calculated.contains(&area) {
                        tree = left;
                        bbox = left_bbox_calculated;
                        direction = direction.next_axis();
                        continue;
                    } else if right_bbox_calculated.contains(&area) {
                        tree = right;
                        bbox = right_bbox_calculated;
                        direction = direction.next_axis();
                        continue;
                    }
                }
                None => {}
            }

            return (tree, bbox, direction);
        }
    }

    /// Find the node which would contain the key. If the key is non-pointlike (e.g. an area,
    /// whose Parent type is equivalent to itself), this may not be a leaf! However,
    /// if the key is stored anywhere in the tree, then it's guaranteed that it will be
    /// in the returned node.
    fn search_leaf_for_key<'a>(
        &'a self,
        k: &Key,
    ) -> (
        &'a Node<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>,
        Key::Parent,
    ) {
        let mut tree = &self.node;

        let mut bbox = self.root_bbox.to_owned();
        let mut direction =
            <Key::Parent as MultidimensionalParent<DIMENSION_COUNT>>::DimensionEnum::arbitrary_first();

        loop {
            match &tree.left_right_split.get() {
                Some((left, right)) => {
                    let (left_bbox_calculated, right_bbox_calculated) =
                        bbox.split_evenly_on_dimension(&direction);

                    if k.is_contained_in(&left_bbox_calculated) {
                        tree = left;
                        bbox = left_bbox_calculated;
                        direction = direction.next_axis();
                        continue;
                    } else if k.is_contained_in(&right_bbox_calculated) {
                        tree = right;
                        bbox = right_bbox_calculated;
                        direction = direction.next_axis();
                        continue;
                    }
                }
                None => {}
            }

            return (&tree, bbox);
        }
    }

    fn get_key_leaf_splitting_if_needed<'a>(
        &'a self,
        k: &Key,
        storage: &impl StoreByPage<
            Inner<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>,
            PageId = PageId<PAGE_SIZE>,
        >,
    ) -> (
        &Node<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>,
        bool,
    ) {
        let mut tree = &self.node;
        let mut direction =
            <Key::Parent as MultidimensionalParent<DIMENSION_COUNT>>::DimensionEnum::arbitrary_first();

        let mut structure_changed = false;

        loop {
            match tree.left_right_split.get() {
                Some((ref left, ref right)) => {
                    if k.is_contained_in(&left.bbox) {
                        tree = left;
                        direction = direction.next_axis();
                        continue;
                    } else if k.is_contained_in(&right.bbox) {
                        tree = right;
                        direction = direction.next_axis();
                        continue;
                    }
                }
                None => {
                    if tree.try_split_left_right(storage, &direction) {
                        structure_changed = true;
                        continue;
                    }
                }
            }

            return (tree, structure_changed);
        }
    }
}

impl<const DIMENSION_COUNT: usize, const NODE_SATURATION_POINT: usize, Key, Value>
    Node<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>
where
    Key: SparseKey<DIMENSION_COUNT>,
    Value: SparseValue,
{
    pub(crate) fn new(
        bbox: Key::Parent,
        storage: &impl StoreByPage<
            Inner<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>,
            PageId = PageId<PAGE_SIZE>,
        >,
    ) -> Self {
        Self::new_with_children(bbox, storage, BTreeVec::new())
    }

    pub(crate) fn new_with_children(
        bbox: Key::Parent,
        storage: &impl StoreByPage<
            Inner<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>,
            PageId = PageId<PAGE_SIZE>,
        >,
        children: BTreeVec<Key, Value>,
    ) -> Self {
        debug_print!("new_with_children called");

        Self {
            bbox: bbox.clone(),
            child_count: children.len().into(),
            page_id: storage.new_page(Inner { children }),
            left_right_split: Default::default(),
            __phantom: std::marker::PhantomData,
        }
    }
    pub fn expand_to_depth(
        &mut self,
        depth: usize,
        bbox: &Key::Parent,
        storage: &impl StoreByPage<
            Inner<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>,
            PageId = PageId<PAGE_SIZE>,
        >,
        direction: &<Key::Parent as MultidimensionalParent<DIMENSION_COUNT>>::DimensionEnum,
    ) {
        self.try_split_left_right(storage, direction);

        if depth > 1 {
            match self.left_right_split.get_mut() {
                Some((ref mut l, ref mut r)) => {
                    l.expand_to_depth(depth - 1, bbox, storage, direction);
                    r.expand_to_depth(depth - 1, bbox, storage, direction);
                }
                None => {}
            }
        }
    }

    fn try_split_left_right(
        &self,
        storage: &impl StoreByPage<
            Inner<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>,
            PageId = PageId<PAGE_SIZE>,
        >,
        direction: &<Key::Parent as MultidimensionalParent<DIMENSION_COUNT>>::DimensionEnum,
    ) -> bool {
        if self.left_right_split.get().is_some() {
            return false;
        }

        debug_print!("try_split_left_right starting, passed the first oncelock check");

        let page = storage.get(&self.page_id, ()).unwrap();
        let inner = page.read();

        debug_print!("got page");

        if inner.children.len() >= NODE_SATURATION_POINT {
            drop(inner);

            return self.split_left_right_unchecked(storage, direction);
        } else {
            return false;
        }
    }
    fn split_left_right_unchecked(
        &self,
        storage: &impl StoreByPage<
            Inner<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>,
            PageId = PageId<PAGE_SIZE>,
        >,
        direction: &<Key::Parent as MultidimensionalParent<DIMENSION_COUNT>>::DimensionEnum,
    ) -> bool {
        debug_print!("split_left_right_unchecked");

        self.left_right_split.get_or_init(|| {
            debug_print!("made it to the inside of the cell init");

            let (left_bb, right_bb) = self.bbox.split_evenly_on_dimension(direction);

            let mut left_children = BTreeVec::new();
            let mut right_children = BTreeVec::new();

            let page = storage.get(&self.page_id, ()).unwrap();

            debug_print!("got page");

            let mut inner = page.write();

            debug_print!("got inner");

            let children = std::mem::take(&mut inner.children);

            //this is messy, but the rationale is as such:
            //if we split our bounding box and nothing changes, then
            //we've reached the end of whatever finite type the user's using
            //as a key. In that case, put 25% of the values into the first
            //(i.e. the left) side and the rest into the last (right) side;
            //this'll ensure balance-ish over time.
            if left_bb == right_bb && left_bb == self.bbox {
                let children_len = children.len();
                let mut children_iter = children.into_iter();

                let first_amnt = children_len / 4;

                left_children = unsafe {
                    BTreeVec::from_sorted_iter_failable::<Infallible>(
                        (&mut children_iter).take(first_amnt).map(Ok),
                    )
                    .unwrap()
                };
                right_children = unsafe {
                    BTreeVec::from_sorted_iter_failable::<Infallible>(
                        children_iter.take(first_amnt).map(Ok),
                    )
                    .unwrap()
                };
            } else {
                for (child_bbox, item) in children.into_iter() {
                    if child_bbox.is_contained_in(&left_bb) {
                        left_children.push(child_bbox, item);
                    } else if child_bbox.is_contained_in(&right_bb) {
                        right_children.push(child_bbox, item);
                    } else {
                        inner.children.push(child_bbox, item);
                    }
                }
            }

            debug_print!("made new splits");

            //Relaxed is appropriate because this code holds a write lock on the corresponding page, so it cannot be modified
            //by any other thread at the same time.
            self.child_count
                .store(inner.children.len(), std::sync::atomic::Ordering::Relaxed);

            drop(inner);

            debug_print!("dropped inner");

            (
                Box::new(Node::new_with_children(left_bb, storage, left_children)),
                Box::new(Node::new_with_children(right_bb, storage, right_children)),
            )
        });

        return true;
    }
}

pub fn make_path(root_path: &PathBuf, id: u64) -> PathBuf {
    let id_hex = format!("{:x}i", id);

    let chunk_size = 4;

    if id_hex.len() < chunk_size {
        root_path.join(id_hex)
    } else if id_hex.len() == chunk_size {
        root_path.join(id_hex).join("i")
    } else {
        root_path
            .join(&id_hex[0..chunk_size])
            .join(&id_hex[chunk_size..])
    }
}
