use std::{
    fs::File,
    io::{Seek, Write},
    path::PathBuf,
};

use btree_vec::{BTreeVec, SeparateStateIteratable};

use crate::{
    dense::structure::{Inner, Node, Root, StoredTree, TreePagedStorage},
    PAGE_SIZE,
};

use crate::tree_traits::{
    Dimension, MultidimensionalKey, MultidimensionalParent, MultidimensionalValue,
};
use minimal_storage::{
    paged_storage::{Page, PageArcReadLock, PageId, PageReadLock, PageRwLock, PagedStorage},
    serialize_min::{DeserializeFromMinimal, SerializeMinimal},
};

impl<const DIMENSION_COUNT: usize, const NODE_SATURATION_POINT: usize, Key, Value>
    StoredTree<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
{
    pub fn new(bbox: Key::Parent, folder: PathBuf) -> Self {
        let storage_file = std::fs::File::options()
            .create(true)
            .read(true)
            .write(true)
            .open(&folder.join("data"))
            .unwrap();
        let mut storage = PagedStorage::open(storage_file);

        let mut structure_file = File::options()
            .write(true)
            .create(true)
            .read(true)
            .open(folder.join("structure"))
            .unwrap();

        let root = if structure_file.metadata().unwrap().len() == 0 {
            let node = Node::new(1, bbox.clone(), &mut storage);

            Root {
                root_bbox: bbox,
                node,
            }
        } else {
            Root::deserialize_minimal(&mut structure_file, &folder).unwrap()
        };

        StoredTree {
            structure_file,
            root,
            structure_dirty: true.into(),
            storage,
        }
    }

    pub fn flush<'s>(&'s mut self) -> std::io::Result<()> {
        if self
            .structure_dirty
            .swap(false, std::sync::atomic::Ordering::Relaxed)
        {
            self.structure_file.rewind().unwrap();

            let mut buf = Vec::new();
            self.root.minimally_serialize(&mut buf, ())?;
            self.structure_file.write_all(&buf).unwrap();
        }

        self.storage.flush();
        Ok(())
    }
    pub fn find_items_in_box<'a>(
        &'a self,
        query: &'a Key::Parent,
    ) -> impl Iterator<Item = Value> + 'a {
        self.find_entries_in_box(query).map(|x| x.1)
    }

    pub fn find_entries_in_box<'a>(
        &'a self,
        query: &'a Key::Parent,
    ) -> impl Iterator<Item = (Key, Value)> + 'a {
        self.root
            .search_all_nodes_touching_area(query)
            .flat_map(move |(node, bbox)| {
                let page_read = Page::read_arc(&self.storage.get(&node.page_id, &bbox).unwrap());

                let mut iter_state = page_read.children.begin_iteration();

                std::iter::from_fn(move || loop {
                    let (iter_state_advance, (k, v)) =
                        page_read.children.stateless_next(iter_state)?;
                    iter_state = iter_state_advance;
                    if Key::delta_from_parent_would_be_contained(&k, &bbox, &query) {
                        let k = Key::apply_delta_from_parent(&k, &bbox);
                        return Some((k, v.to_owned()));
                    }
                })
            })
    }

    pub fn find_first_item_at_key_exact<'a, 'b>(&'a self, query: &'b Key) -> Option<Value> {
        let (leaf, leaf_bbox) = self.root.search_leaf_for_key(query);

        let delta = query.delta_from_parent(&leaf_bbox);

        let page = self.storage.get(&leaf.page_id, &leaf_bbox).unwrap();

        let item = page.read().children.get(&delta)?.iter().next().cloned();

        item
    }

    pub fn insert(&self, k: &Key, item: Value) {
        let (leaf, leaf_bbox, structure_changed) =
            self.root.get_key_leaf_splitting_if_needed(k, &self.storage);

        let interior_delta_bbox = k.delta_from_parent(&leaf_bbox);
        self.storage
            .get(&leaf.page_id, &leaf_bbox)
            .unwrap()
            .write()
            .children
            .push(interior_delta_bbox, item);

        leaf.children_count
            .fetch_add(1, std::sync::atomic::Ordering::Acquire);

        self.structure_dirty
            .fetch_or(true, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn expand_to_depth(&mut self, depth: usize) {
        self.root.node.expand_to_depth(
            depth,
            &self.root.root_bbox,
            &mut self.storage,
            &Default::default(),
        )
    }
}

impl<const DIMENSION_COUNT: usize, const NODE_SATURATION_POINT: usize, Key, Value>
    Root<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
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
            <Key::Parent as MultidimensionalParent<DIMENSION_COUNT>>::DimensionEnum::default();

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
            <Key::Parent as MultidimensionalParent<DIMENSION_COUNT>>::DimensionEnum::default();

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
        storage: &TreePagedStorage<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>,
    ) -> (
        &Node<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>,
        Key::Parent,
        bool,
    ) {
        let mut tree = &self.node;
        let mut direction =
            <Key::Parent as MultidimensionalParent<DIMENSION_COUNT>>::DimensionEnum::default();

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

            return (tree, tree.bbox.to_owned(), structure_changed);
        }
    }
}

impl<const DIMENSION_COUNT: usize, const NODE_SATURATION_POINT: usize, Key, Value>
    Node<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
{
    pub(crate) fn new(
        id: u64,
        bbox: Key::Parent,
        storage: &TreePagedStorage<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>,
    ) -> Self {
        Self::new_with_children(id, bbox, storage, BTreeVec::new())
    }

    pub(crate) fn new_with_children(
        id: u64,
        bbox: Key::Parent,
        storage: &TreePagedStorage<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>,
        children: BTreeVec<Key::DeltaFromParent, Value>,
    ) -> Self {
        let children_count = children.len().into();
        Self {
            bbox: bbox.clone(),
            page_id: storage.new_page(Inner { children }),
            children_count,
            left_right_split: Default::default(),
            id,
            __phantom: std::marker::PhantomData,
        }
    }
    pub fn expand_to_depth(
        &mut self,
        depth: usize,
        bbox: &Key::Parent,
        storage: &TreePagedStorage<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>,
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
        storage: &TreePagedStorage<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>,
        direction: &<Key::Parent as MultidimensionalParent<DIMENSION_COUNT>>::DimensionEnum,
    ) -> bool {
        if self.left_right_split.get().is_some() {
            return false;
        }

        let len = self
            .children_count
            .fetch_min(NODE_SATURATION_POINT, std::sync::atomic::Ordering::Relaxed);

        if len > NODE_SATURATION_POINT {
            return self.split_left_right_unchecked(storage, direction);
        } else {
            return false;
        }
    }
    fn split_left_right_unchecked(
        &self,
        storage: &TreePagedStorage<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>,
        direction: &<Key::Parent as MultidimensionalParent<DIMENSION_COUNT>>::DimensionEnum,
    ) -> bool {
        self.left_right_split.get_or_init(|| {
            let (left_bb, right_bb) = self.bbox.split_evenly_on_dimension(direction);

            let mut left_children = BTreeVec::new();
            let mut right_children = BTreeVec::new();

            let page = storage.get(&self.page_id, &self.bbox).unwrap();

            let mut inner = page.write();

            let children = std::mem::take(&mut inner.children);

            for (child_bbox, item) in children.into_iter() {
                let bb_abs = Key::apply_delta_from_parent(&child_bbox, &self.bbox);

                if bb_abs.is_contained_in(&left_bb) {
                    left_children.push(bb_abs.delta_from_parent(&left_bb), item);
                } else if bb_abs.is_contained_in(&right_bb) {
                    right_children.push(bb_abs.delta_from_parent(&right_bb), item);
                } else {
                    inner.children.push(child_bbox, item);
                }
            }

            self.children_count
                .store(inner.children.len(), std::sync::atomic::Ordering::SeqCst);

            let (left_id, right_id) = split_id(self.id);

            (
                Box::new(Node::new_with_children(
                    left_id,
                    left_bb,
                    storage,
                    left_children,
                )),
                Box::new(Node::new_with_children(
                    right_id,
                    right_bb,
                    storage,
                    right_children,
                )),
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

pub(super) fn split_id(id: u64) -> (u64, u64) {
    ((id << 1), (id << 1) | 1)
}
