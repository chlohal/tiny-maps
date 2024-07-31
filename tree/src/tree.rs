use std::{path::PathBuf, rc::Rc};

use btree_vec::BTreeVec;

use super::{
    compare_by::OrderByFirst,
    point_range::DisregardWhenDeserializing,
    tree_traits::{Dimension, MultidimensionalKey, MultidimensionalParent, MultidimensionalValue},
    NODE_SATURATION_POINT,
};
use minimal_storage::{serialize_min::SerializeMinimal, Storage};

pub type StoredPointTree<const D: usize, K, T> = Storage<
    (RootTreeInfo, u64, <K as MultidimensionalKey<D>>::Parent),
    LongLatTree<D, K, DisregardWhenDeserializing<K, T>>,
>;

pub type StoredTree<const D: usize, K, T> =
    Storage<(RootTreeInfo, u64, <K as MultidimensionalKey<D>>::Parent), LongLatTree<D, K, T>>;

pub struct LongLatTree<const DIMENSION_COUNT: usize, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
    for<'serialize> <Value as SerializeMinimal>::ExternalData<'serialize>: Copy,
{
    pub(super) root_tree_info: RootTreeInfo,
    pub(super) bbox: Key::Parent,
    pub(super) direction:
        <<Key as MultidimensionalKey<DIMENSION_COUNT>>::Parent as MultidimensionalParent<
            DIMENSION_COUNT,
        >>::DimensionEnum,
    pub(super) children: BTreeVec<Key::DeltaFromParent, Value>,
    pub(super) left_right_split: Option<(
        StoredTree<DIMENSION_COUNT, Key, Value>,
        StoredTree<DIMENSION_COUNT, Key, Value>,
    )>,
    pub(super) id: u64,
}

pub(super) type RootTreeInfo = Rc<PathBuf>;

pub struct LongLatTreeItems<'a, const DIMENSION_COUNT: usize, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
    for<'serialize> <Value as SerializeMinimal>::ExternalData<'serialize>: Copy,
{
    query_bbox: &'a Key::Parent,
    parent_tree_stack: Vec<&'a StoredTree<DIMENSION_COUNT, Key, Value>>,
    current_tree_children: (
        &'a Key::Parent,
        btree_vec::Iter<'a, Key::DeltaFromParent, Value>,
    ),
}

impl<'a, const DIMENSION_COUNT: usize, Key, Value> Iterator
    for LongLatTreeItems<'a, DIMENSION_COUNT, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
    for<'serialize> <Value as SerializeMinimal>::ExternalData<'serialize>: Copy,
{
    type Item = &'a Value;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(next) = self.current_tree_children.1.next() {
                let key = Key::apply_delta_from_parent(&next.0, &self.current_tree_children.0);
                if key.is_contained_in(&self.query_bbox) {
                    return Some(&next.1);
                }
            } else {
                let tree = self.parent_tree_stack.pop()?.deref();
                if let Some((ref l, ref r)) = tree.left_right_split {
                    self.parent_tree_stack.push(l);
                    self.parent_tree_stack.push(r);
                }
                self.current_tree_children = (&tree.bbox, tree.children.iter());
            }
        }
    }
}

pub struct LongLatTreeEntries<'a, const DIMENSION_COUNT: usize, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
    for<'serialize> <Value as SerializeMinimal>::ExternalData<'serialize>: Copy,
{
    query_bbox: &'a Key::Parent,
    parent_tree_stack: Vec<&'a StoredTree<DIMENSION_COUNT, Key, Value>>,
    current_tree_children: (
        &'a Key::Parent,
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
            if let Some(next) = self.current_tree_children.1.next() {
                let key = Key::apply_delta_from_parent(&next.0, &self.current_tree_children.0);
                if key.is_contained_in(&self.query_bbox) {
                    return Some((key.clone(), &next.1));
                }
            } else {
                let tree = self.parent_tree_stack.pop()?.deref();
                if let Some((ref l, ref r)) = tree.left_right_split {
                    self.parent_tree_stack.push(l);
                    self.parent_tree_stack.push(r);
                }
                self.current_tree_children = (&tree.bbox, tree.children.iter());
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
    pub fn new(bbox: Key::Parent, root_tree_info: RootTreeInfo) -> Self {
        LongLatTree {
            root_tree_info,
            bbox,
            direction:
                <Key::Parent as MultidimensionalParent<DIMENSION_COUNT>>::DimensionEnum::default(),
            children: BTreeVec::new(),
            left_right_split: None,
            id: 1,
        }
    }
    pub fn find_first_item_at_key_exact<'a, 'b>(&'a self, query: &'b Key) -> Option<&'a Value> {
        let mut tree = self;

        let (leaf, leaf_bbox) = loop {
            match tree.left_right_split {
                Some((ref left, ref right)) => {
                    if query.is_contained_in(&left.deref().bbox) {
                        tree = left.deref();
                        continue;
                    } else if query.is_contained_in(&right.deref().bbox) {
                        tree = right.deref();
                        continue;
                    }
                }
                None => {}
            }

            break (&tree.children, &tree.bbox);
        };

        let delta = query.delta_from_parent(leaf_bbox);

        leaf.iter().filter(|x| *x.0 == delta).map(|x| x.1).next()
    }

    pub fn find_items_in_box<'a>(
        &'a self,
        query_bbox: &'a Key::Parent,
    ) -> LongLatTreeItems<'a, DIMENSION_COUNT, Key, Value> {
        match &self.left_right_split {
            Some((left, right)) => {
                let l = left.deref();
                let r = right.deref();

                if l.bbox.contains(query_bbox) {
                    return l.find_items_in_box(query_bbox);
                } else if r.bbox.contains(query_bbox) {
                    return r.find_items_in_box(query_bbox);
                } else {
                    return LongLatTreeItems {
                        query_bbox,
                        parent_tree_stack: vec![left, right],
                        current_tree_children: (&self.bbox, self.children.iter()),
                    };
                }
            }
            None => {
                return LongLatTreeItems {
                    query_bbox,
                    parent_tree_stack: Vec::with_capacity(0),
                    current_tree_children: (&self.bbox, self.children.iter()),
                }
            }
        }
    }

    pub fn find_entries_in_box<'a>(
        &'a self,
        query_bbox: &'a Key::Parent,
    ) -> LongLatTreeEntries<'a, DIMENSION_COUNT, Key, Value> {
        match &self.left_right_split {
            Some((left, right)) => {
                let l = left.deref();
                let r = right.deref();

                if l.bbox.contains(query_bbox) {
                    return l.find_entries_in_box(query_bbox);
                } else if r.bbox.contains(query_bbox) {
                    return r.find_entries_in_box(query_bbox);
                } else {
                    return LongLatTreeEntries {
                        query_bbox,
                        parent_tree_stack: vec![left, right],
                        current_tree_children: (&self.bbox, self.children.iter()),
                    };
                }
            }
            None => {
                return LongLatTreeEntries {
                    query_bbox,
                    parent_tree_stack: Vec::with_capacity(0),
                    current_tree_children: (&self.bbox, self.children.iter()),
                }
            }
        }
    }

    pub fn insert(&mut self, k: &Key, item: Value) {
        let mut tree = self;

        let (leaf, leaf_bbox) = loop {
            match tree.left_right_split {
                Some((ref mut left, ref mut right)) => {
                    if k.is_contained_in(&left.deref().bbox) {
                        tree = left.ref_mut();
                        continue;
                    } else if k.is_contained_in(&right.deref().bbox) {
                        tree = right.ref_mut();
                        continue;
                    }
                }
                None => {
                    if tree.children.len() >= NODE_SATURATION_POINT {
                        tree.split_left_right();
                        continue;
                    }
                }
            }

            break (&mut tree.children, &mut tree.bbox);
        };

        let interior_delta_bbox = k.delta_from_parent(&leaf_bbox);
        leaf.push(interior_delta_bbox, item);
    }

    pub fn expand_to_depth(&mut self, depth: usize) {
        if self.left_right_split.is_none() {
            self.split_left_right();
        }

        if depth > 1 {
            match self.left_right_split {
                Some((ref mut l, ref mut r)) => {
                    l.ref_mut().expand_to_depth(depth - 1);
                    r.ref_mut().expand_to_depth(depth - 1);
                }
                None => unreachable!(),
            }
        }
    }

    fn split_left_right(&mut self) {
        debug_assert!(self.left_right_split.is_none());

        let (left_bb, right_bb) = self.bbox.split_evenly_on_dimension(&self.direction);

        let mut left_children = BTreeVec::new();
        let mut right_children = BTreeVec::new();

        let children = std::mem::take(&mut self.children);

        for (child_bbox, item) in children.into_iter() {
            let bb_abs = Key::apply_delta_from_parent(&child_bbox, &self.bbox);

            if bb_abs.is_contained_in(&left_bb) {
                left_children.push(bb_abs.delta_from_parent(&left_bb), item);
            } else if bb_abs.is_contained_in(&right_bb) {
                right_children.push(bb_abs.delta_from_parent(&right_bb), item);
            } else {
                self.children.push(child_bbox, item);
            }
        }

        let (left_path, left_id) = self.make_branch_id(0);
        let (right_path, right_id) = self.make_branch_id(1);

        self.left_right_split = Some((
            StoredTree::new(
                left_path,
                LongLatTree {
                    root_tree_info: Rc::clone(&self.root_tree_info),
                    bbox: left_bb.clone(),
                    direction: self.direction.next_axis(),
                    children: left_children,
                    left_right_split: None,
                    id: left_id,
                },
                (Rc::clone(&self.root_tree_info), left_id, left_bb),
            ),
            StoredTree::new(
                right_path,
                LongLatTree {
                    root_tree_info: Rc::clone(&self.root_tree_info),
                    bbox: right_bb.clone(),
                    direction: self.direction.next_axis(),
                    children: right_children,
                    left_right_split: None,
                    id: right_id,
                },
                (Rc::clone(&self.root_tree_info), right_id, right_bb),
            ),
        ))
    }

    fn make_branch_id(&self, direction: u64) -> (PathBuf, u64) {
        return branch_id_creation(&self.root_tree_info, self.id, direction);
    }
}

pub(super) fn branch_id_creation(
    root_path: &RootTreeInfo,
    id: u64,
    direction_bit: u64,
) -> (PathBuf, u64) {
    let new_id = (id << 1) | direction_bit;

    let info = root_path.join(format!("{:x}", new_id));

    (info, new_id)
}

pub trait UpdateOnBasisLongLatMove {
    fn update(&mut self, old_basis: Self, new_basis: Self);
}
