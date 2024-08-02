use std::{
    fs::File,
    io::{Seek, Write},
    ops::Deref,
    path::PathBuf,
};

use btree_vec::BTreeVec;

use crate::structure::{Inner, LongLatTree, Node, Root, StoredChildren};

use super::{
    point_range::DisregardWhenDeserializing,
    tree_traits::{Dimension, MultidimensionalKey, MultidimensionalParent, MultidimensionalValue},
    NODE_SATURATION_POINT,
};
use minimal_storage::{serialize_min::{DeserializeFromMinimal, SerializeMinimal}, Storage};

pub type StoredPointTree<const D: usize, K, T> =
    LongLatTree<D, K, DisregardWhenDeserializing<K, T>>;

pub type StoredTree<const D: usize, K, T> = LongLatTree<D, K, T>;

pub struct LongLatTreeEntries<'a, const DIMENSION_COUNT: usize, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
    for<'serialize> <Value as SerializeMinimal>::ExternalData<'serialize>: Copy,
{
    query_bbox: &'a Key::Parent,
    parent_tree_stack: Vec<(
        Key::Parent,
        &'a Node<DIMENSION_COUNT, Key, Value>,
        <Key::Parent as MultidimensionalParent<DIMENSION_COUNT>>::DimensionEnum,
    )>,
    current_tree_children: (
        Key::Parent,
        btree_vec::Iter<'a, Key::DeltaFromParent, Value>,
    ),
}

impl<'a, const DIMENSION_COUNT: usize, Key, Value> Iterator
    for LongLatTreeEntries<'a, DIMENSION_COUNT, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
    for<'serialize> <Value as SerializeMinimal>::ExternalData<'serialize>: Copy,
{
    type Item = (Key, &'a Value);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some((delta, value)) = self.current_tree_children.1.next() {
                let key = Key::apply_delta_from_parent(&delta, &self.current_tree_children.0);
                
                if key.is_contained_in(&self.query_bbox) {
                    return Some((key.clone(), &value));
                }
            } else {
                let (bbox, tree, direction) = self.parent_tree_stack.pop()?;

                if let Some((ref l, ref r)) = tree.left_right_split {
                    let (lbox, rbox) = bbox.split_evenly_on_dimension(&direction);
                    let dir = direction.next_axis();

                    self.parent_tree_stack.push((lbox, l, dir));
                    self.parent_tree_stack.push((rbox, r, dir));
                }
                self.current_tree_children = (bbox, tree.values.deref().children.iter());
            }
        }
    }
}

impl<const DIMENSION_COUNT: usize, Key, Value> LongLatTree<DIMENSION_COUNT, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
    for<'serialize> <Value as SerializeMinimal>::ExternalData<'serialize>: Copy,
{
    pub fn new(bbox: Key::Parent, folder: PathBuf) -> Self {
        let node = Node::new(1, bbox.clone(), &folder);

        let mut structure_file = File::options()
            .write(true)
            .create(true)
            .read(true)
            .open(folder.join("structure"))
            .unwrap();

        let root = if structure_file.metadata().unwrap().len() == 0 {
            Root {
                root_bbox: bbox,
                node,
            }
        } else {
            Root::deserialize_minimal(&mut structure_file, &folder).unwrap()
        };

        LongLatTree {
            structure_file,
            storage_folder: folder,
            root,
            structure_dirty: true,
        }
    }
    pub fn flush<'s>(
        &'s mut self,
        serialize_data: <Value as SerializeMinimal>::ExternalData<'s>,
    ) -> std::io::Result<()> {
        if self.structure_dirty {
            self.structure_file.rewind().unwrap();

            let mut buf = Vec::new();
            self.root.minimally_serialize(&mut buf, ()).unwrap();
            self.structure_file.write_all(&buf).unwrap();

            self.structure_dirty = false;
        }

        for nodevalues in self.root.nodes() {
            match nodevalues.flush(serialize_data) {
                Some(Err(e)) => return Err(e),
                None | Some(Ok(())) => {}
            }
        }

        Ok(())
    }
    pub fn find_first_item_at_key_exact<'a, 'b>(&'a self, query: &'b Key) -> Option<&'a Value> {
        let (leaf, leaf_bbox) = self.root.search_leaf_for_key(query);

        let delta = query.delta_from_parent(&leaf_bbox);

        leaf.values.deref().children.get(&delta)?.iter().next()
    }

    pub fn find_items_in_box<'a>(
        &'a self,
        query_bbox: &'a Key::Parent,
    ) -> impl Iterator<Item = &'a Value> + 'a {
        self.find_entries_in_box(query_bbox).map(|x| x.1)
    }

    pub fn find_entries_in_box<'a>(
        &'a self,
        query_bbox: &'a Key::Parent,
    ) -> LongLatTreeEntries<'a, DIMENSION_COUNT, Key, Value> {
        let (leaf, leaf_bbox, direction) = self.root.search_leaf_inside_parent(query_bbox);

        LongLatTreeEntries {
            query_bbox,
            parent_tree_stack: leaf
                .left_right_split
                .as_ref()
                .map(|(l, r)| {
                    let (lbox, rbox) = leaf_bbox.split_evenly_on_dimension(&direction);
                    let dir = direction.next_axis();

                    vec![(lbox, l.deref(), dir), (rbox, r.deref(), dir)]
                })
                .unwrap_or_default(),
            current_tree_children: (leaf_bbox.to_owned(), leaf.values.deref().children.iter()),
        }
    }

    pub fn insert(&mut self, k: &Key, item: Value) {
        let (leaf, leaf_bbox, structure_changed) = self
            .root
            .get_key_leaf_splitting_if_needed(k, &self.storage_folder);

        let interior_delta_bbox = k.delta_from_parent(&leaf_bbox);
        leaf.push(interior_delta_bbox, item);

        self.structure_dirty |= structure_changed;
    }

    pub fn expand_to_depth(&mut self, depth: usize) {
        self.root.node.expand_to_depth(
            depth,
            &self.root.root_bbox,
            &self.storage_folder,
            &Default::default(),
        )
    }
}

struct NodesMut<'a, const DIMENSION_COUNT: usize, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
    for<'serialize> <Value as SerializeMinimal>::ExternalData<'serialize>: Copy,
{
    node_stack: Vec<&'a mut Node<DIMENSION_COUNT, Key, Value>>,
}

impl<'a, const DIMENSION_COUNT: usize, Key, Value> Iterator
    for NodesMut<'a, DIMENSION_COUNT, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
    for<'serialize> <Value as SerializeMinimal>::ExternalData<'serialize>: Copy,
{
    type Item = &'a mut Storage<
        <Key as MultidimensionalKey<DIMENSION_COUNT>>::Parent,
        Inner<DIMENSION_COUNT, Key, Value>,
    >;

    fn next(&mut self) -> Option<Self::Item> {
        let n = self.node_stack.pop()?;

        match &mut n.left_right_split {
            Some((l, r)) => {
                self.node_stack.push(l);
                self.node_stack.push(r);
            }
            None => {}
        }

        Some(&mut n.values)
    }
}

impl<const DIMENSION_COUNT: usize, Key, Value> Root<DIMENSION_COUNT, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
    for<'serialize> <Value as SerializeMinimal>::ExternalData<'serialize>: Copy,
{
    pub(self) fn nodes<'a>(&'a mut self) -> NodesMut<'a, DIMENSION_COUNT, Key, Value> {
        NodesMut {
            node_stack: vec![&mut self.node],
        }
    }

    fn search_leaf_for_key<'a>(
        &'a self,
        k: &Key,
    ) -> (&'a Node<DIMENSION_COUNT, Key, Value>, Key::Parent) {
        let mut tree = &self.node;

        let mut bbox = self.root_bbox.to_owned();
        let mut direction =
            <Key::Parent as MultidimensionalParent<DIMENSION_COUNT>>::DimensionEnum::default();

        loop {
            match &tree.left_right_split {
                Some((left,  right)) => {
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

    fn search_leaf_inside_parent<'a>(
        &'a self,
        k: &Key::Parent,
    ) -> (
        &'a Node<DIMENSION_COUNT, Key, Value>,
        Key::Parent,
        <Key::Parent as MultidimensionalParent<DIMENSION_COUNT>>::DimensionEnum,
    ) {
        let mut tree = &self.node;

        let mut bbox = self.root_bbox.clone();
        let mut direction =
            <Key::Parent as MultidimensionalParent<DIMENSION_COUNT>>::DimensionEnum::default();

        loop {
            match &tree.left_right_split {
                Some((left, right)) => {
                    let (left_bbox_calculated, right_bbox_calculated) =
                        bbox.split_evenly_on_dimension(&direction);

                    if left_bbox_calculated.contains(k) {
                        tree = left;
                        bbox = left_bbox_calculated;
                        direction = direction.next_axis();
                        continue;
                    } else if right_bbox_calculated.contains(k) {
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

    fn get_key_leaf_splitting_if_needed<'a>(
        &'a mut self,
        k: &Key,
        root_path: &PathBuf,
    ) -> (
        &'a mut BTreeVec<Key::DeltaFromParent, Value>,
        Key::Parent,
        bool,
    ) {
        let mut tree = &mut self.node;
        let mut bbox = self.root_bbox.clone();
        let mut direction =
            <Key::Parent as MultidimensionalParent<DIMENSION_COUNT>>::DimensionEnum::default();

        let mut structure_changed = false;

        loop {
            match tree.left_right_split {
                Some((ref mut left, ref mut right)) => {
                    let (left_bbox_calculated, right_bbox_calculated) =
                        bbox.split_evenly_on_dimension(&direction);

                    if k.is_contained_in(&left.bbox) {
                        tree = left;
                         bbox = left_bbox_calculated;
                        direction = direction.next_axis();
                        continue;
                    } else if k.is_contained_in(&right.bbox) {
                        tree = right;
                        bbox = right_bbox_calculated;
                        direction = direction.next_axis();
                        continue;
                    }
                }
                None => {
                    if tree.try_split_left_right(&bbox, root_path, &direction) {
                        structure_changed = true;
                        continue;
                    }
                }
            }

            return (&mut tree.values.ref_mut().children, bbox, structure_changed);
        }
    }
}

impl<const DIMENSION_COUNT: usize, Key, Value> Node<DIMENSION_COUNT, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
    for<'serialize> <Value as SerializeMinimal>::ExternalData<'serialize>: Copy,
{
    pub(crate) fn new(id: u64, bbox: Key::Parent, root_path: &PathBuf) -> Self {
        Self::new_with_children(id, bbox, root_path, BTreeVec::new())
    }

    pub(crate) fn new_with_children(id: u64, bbox: Key::Parent, root_path: &PathBuf, children: BTreeVec<Key::DeltaFromParent, Value>) -> Self {
        let my_path = make_path(root_path, id);
        Self {
            bbox: bbox.clone(),
            values: StoredChildren::new(my_path, Inner {
                children,
            }, bbox),
            left_right_split: None,
            id,
        }
    }
    pub fn expand_to_depth(
        &mut self,
        depth: usize,
        bbox: &Key::Parent,
        root_path: &PathBuf,
        direction: &<Key::Parent as MultidimensionalParent<DIMENSION_COUNT>>::DimensionEnum,
    ) {
        self.try_split_left_right(bbox, root_path, direction);

        if depth > 1 {
            match self.left_right_split {
                Some((ref mut l, ref mut r)) => {
                    l.expand_to_depth(depth - 1, bbox, root_path, direction);
                    r.expand_to_depth(depth - 1, bbox, root_path, direction);
                }
                None => {}
            }
        }
    }

    fn try_split_left_right(
        &mut self,
        bbox: &Key::Parent,
        root_path: &PathBuf,
        direction: &<Key::Parent as MultidimensionalParent<DIMENSION_COUNT>>::DimensionEnum,
    ) -> bool {
        if self.left_right_split.is_some() {
            return false;
        }

        let inner = self.values.deref();

        if inner.children.len() >= NODE_SATURATION_POINT {
            return self.split_left_right_unchecked(bbox, direction, root_path);
        } else {
            return false;
        }
    }
    fn split_left_right_unchecked(
        &mut self,
        bbox: &Key::Parent,
        direction: &<Key::Parent as MultidimensionalParent<DIMENSION_COUNT>>::DimensionEnum,
        root_path: &PathBuf,
    ) -> bool {
        let (left_bb, right_bb) = bbox.split_evenly_on_dimension(direction);

        let mut left_children = BTreeVec::new();
        let mut right_children = BTreeVec::new();

        let inner = self.values.ref_mut();

        let children = std::mem::take(&mut inner.children);

        for (child_bbox, item) in children.into_iter() {
            let bb_abs = Key::apply_delta_from_parent(&child_bbox, &bbox);

            if bb_abs.is_contained_in(&left_bb) {
                left_children.push(bb_abs.delta_from_parent(&left_bb), item);
            } else if bb_abs.is_contained_in(&right_bb) {
                right_children.push(bb_abs.delta_from_parent(&right_bb), item);
            } else {
                inner.children.push(child_bbox, item);
            }
        }

        let (left_id, right_id) = split_id(self.id);

        self.left_right_split = Some((
            Box::new(Node::new_with_children(left_id, left_bb, root_path, left_children)),
            Box::new(Node::new_with_children(right_id, right_bb, root_path, right_children)),
        ));

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
        root_path.join(&id_hex[0..chunk_size]).join(&id_hex[chunk_size..])
    }
}

pub(super) fn split_id(id: u64) -> (u64, u64) {
    ((id << 1), (id << 1) | 1)
}