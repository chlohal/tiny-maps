use std::{fs::File, path::PathBuf, rc::Rc};

use super::{
    bbox::{BoundingBox, DeltaBoundingBox, LongLatSplitDirection},
    NODE_SATURATION_POINT,
};
use crate::storage::{
    serialize_min::{DeserializeFromMinimal, SerializeMinimal},
    Storage,
};

pub type StoredTree<T> = Storage<(RootTreeInfo, u64, BoundingBox<i32>), LongLatTree<T>>;

pub struct LongLatTree<T>
where
    T: 'static
        + SerializeMinimal
        + for<'a> DeserializeFromMinimal<ExternalData<'a> = &'a BoundingBox<i32>>,
    for<'a> <T as SerializeMinimal>::ExternalData<'a>: Copy,
{
    pub(super) root_tree_info: RootTreeInfo,
    pub(super) bbox: BoundingBox<i32>,
    pub(super) direction: LongLatSplitDirection,
    pub(super) children: Vec<(DeltaBoundingBox<u32>, T)>,
    pub(super) left_right_split: Option<(StoredTree<T>, StoredTree<T>)>,
    pub(super) id: u64,
}

impl<T> crate::storage::StorageReachable for LongLatTree<T>
where
    T: 'static
        + SerializeMinimal
        + for<'a> DeserializeFromMinimal<ExternalData<'a> = &'a BoundingBox<i32>>,
    for<'a> <T as SerializeMinimal>::ExternalData<'a>: Copy,
{
    fn flush_children<'a>(
        &'a self,
        data: <T as SerializeMinimal>::ExternalData<'a>,
    ) -> Result<(), std::io::Error> {
        match &self.left_right_split {
            Some((l, r)) => {
                match l.flush(data) {
                    Some(Err(e)) => return Err(e),
                    Some(Ok(())) | None => {}
                }
                match r.flush(data) {
                    Some(Err(e)) => return Err(e),
                    Some(Ok(())) | None => {}
                }
                Ok(())
            }
            None => Ok(()),
        }
    }
}

pub(super) type RootTreeInfo = Rc<(PathBuf, File)>;

pub struct LongLatTreeItems<'a, T>
where
    T: 'static
        + SerializeMinimal
        + for<'d> DeserializeFromMinimal<ExternalData<'d> = &'d BoundingBox<i32>>,
    for<'s> <T as SerializeMinimal>::ExternalData<'s>: Copy,
{
    query_bbox: &'a BoundingBox<i32>,
    parent_tree_stack: Vec<&'a StoredTree<T>>,
    current_tree_children: (
        BoundingBox<i32>,
        std::slice::Iter<'a, (DeltaBoundingBox<u32>, T)>,
    ),
}

impl<'a, T> Iterator for LongLatTreeItems<'a, T>
where
    T: 'static
        + SerializeMinimal
        + for<'d> DeserializeFromMinimal<ExternalData<'d> = &'d BoundingBox<i32>>,
    for<'s> <T as SerializeMinimal>::ExternalData<'s>: Copy,
{
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(next) = self.current_tree_children.1.next() {
                if self
                    .query_bbox
                    .contains(&next.0.absolute(&self.current_tree_children.0))
                {
                    return Some(&next.1);
                }
            } else {
                let tree = self.parent_tree_stack.pop()?.deref();
                if let Some((ref l, ref r)) = tree.left_right_split {
                    self.parent_tree_stack.push(l);
                    self.parent_tree_stack.push(r);
                }
                self.current_tree_children = (tree.bbox, tree.children.iter());
            }
        }
    }
}

impl<T> LongLatTree<T>
where
    T: 'static
        + SerializeMinimal
        + for<'a> DeserializeFromMinimal<ExternalData<'a> = &'a BoundingBox<i32>>,
    for<'s> <T as SerializeMinimal>::ExternalData<'s>: Copy,
{
    pub fn new(bbox: BoundingBox<i32>, root_tree_info: RootTreeInfo) -> Self {
        LongLatTree {
            root_tree_info,
            bbox,
            direction: LongLatSplitDirection::default(),
            children: Vec::new(),
            left_right_split: None,
            id: 1,
        }
    }
    pub fn find_items_in_box<'a>(
        &'a self,
        query_bbox: &'a BoundingBox<i32>,
    ) -> LongLatTreeItems<'a, T> {
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
                        current_tree_children: (self.bbox, self.children.iter()),
                    };
                }
            }
            None => {
                return LongLatTreeItems {
                    query_bbox,
                    parent_tree_stack: Vec::with_capacity(0),
                    current_tree_children: (self.bbox, self.children.iter()),
                }
            }
        }
    }

    pub fn insert(&mut self, bbox: BoundingBox<i32>, item: T) {
        match &mut self.left_right_split {
            Some((left, right)) => {
                if left.deref().bbox.contains(&bbox) {
                    left.modify(|tree| tree.insert(bbox, item));
                    return;
                } else if right.deref().bbox.contains(&bbox) {
                    right.modify(|tree| tree.insert(bbox, item));
                    return;
                } else {
                    self.children.push((bbox.interior_delta(&self.bbox), item));
                }
            }
            None => {
                if self.children.len() < NODE_SATURATION_POINT {
                    self.children.push((bbox.interior_delta(&self.bbox), item));
                } else {
                    self.split_left_right();
                    return self.insert(bbox, item);
                }
            }
        }
    }

    pub fn expand_to_depth(&mut self, depth: usize) {
        if self.left_right_split.is_none() {
            self.split_left_right();
        }

        if depth > 1 {
            match self.left_right_split {
                Some((ref mut l, ref mut r)) => {
                    l.modify(|tree| tree.expand_to_depth(depth - 1));
                    r.modify(|tree| tree.expand_to_depth(depth - 1));
                }
                None => unreachable!(),
            }
        }
    }

    fn split_left_right(&mut self) {
        debug_assert!(self.left_right_split.is_none());

        let (left_bb, right_bb) = self.bbox.split_on_axis(&self.direction);

        let mut left_children = Vec::new();
        let mut both_children = Vec::new();
        let mut right_children = Vec::new();

        while let Some((bbox, item)) = self.children.pop() {
            let bb_abs = bbox.absolute(&self.bbox);

            if left_bb.contains(&bb_abs) {
                left_children.push((bb_abs.interior_delta(&left_bb), item));
            } else if right_bb.contains(&bb_abs) {
                right_children.push((bb_abs.interior_delta(&right_bb), item));
            } else {
                both_children.push((bbox, item));
            }
        }

        self.children.extend(both_children);

        let (left_path, left_id) = self.make_branch_id(0);
        let (right_path, right_id) = self.make_branch_id(1);

        self.left_right_split = Some((
            StoredTree::new(
                left_path,
                LongLatTree {
                    root_tree_info: Rc::clone(&self.root_tree_info),
                    bbox: left_bb,
                    direction: !self.direction,
                    children: left_children,
                    left_right_split: None,
                    id: left_id,
                },
                (Rc::clone(&self.root_tree_info), left_id, right_bb),
            ),
            StoredTree::new(
                right_path,
                LongLatTree {
                    root_tree_info: Rc::clone(&self.root_tree_info),
                    bbox: right_bb,
                    direction: !self.direction,
                    children: right_children,
                    left_right_split: None,
                    id: right_id,
                },
                (Rc::clone(&self.root_tree_info), right_id, left_bb),
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

    let info = root_path.0.join(format!("{:x}", new_id));

    (info, new_id)
}

pub trait UpdateOnBasisLongLatMove {
    fn update(&mut self, old_basis: Self, new_basis: Self);
}
