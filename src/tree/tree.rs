use std::{
    collections::{btree_set, BTreeSet},
    mem,
    path::PathBuf,
    rc::Rc,
};

use super::{bbox::{BoundingBox, LongLatSplitDirection}, NODE_SATURATION_POINT};
use super::compare_by::BoundingBoxOrderedByXOrY;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use crate::storage::Storage;
pub struct LongLatTree<T>
where
    T: Serialize + DeserializeOwned,
{
    pub(super) root_tree_info: Rc<RootTreeInfo>,
    pub(super) bbox: BoundingBox<i32>,
    pub(super) direction: LongLatSplitDirection,
    pub(super) children: BTreeSet<BoundingBoxOrderedByXOrY<i32, T>>,
    pub(super) left_right_split: Option<(Storage<LongLatTree<T>>, Storage<LongLatTree<T>>)>,
    pub(super) id: u64,
}

#[derive(Deserialize, Serialize)]
pub(super) struct RootTreeInfo(PathBuf);

pub struct LongLatTreeItems<'a, T: Serialize + DeserializeOwned> {
    query_bbox: &'a BoundingBox<i32>,
    parent_tree_stack: Vec<&'a Storage<LongLatTree<T>>>,
    current_tree_children: btree_set::Iter<'a, BoundingBoxOrderedByXOrY<i32, T>>,
}

impl<'a, T: Serialize + DeserializeOwned> Iterator for LongLatTreeItems<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(next) = self.current_tree_children.next() {
                if self.query_bbox.contains(&next.0) {
                    return Some(&next.2);
                }
            } else {
                let tree = self.parent_tree_stack.pop()?.deref();
                if let Some((ref l, ref r)) = tree.left_right_split {
                    self.parent_tree_stack.push(l);
                    self.parent_tree_stack.push(r);
                }
                self.current_tree_children = tree.children.iter();
            }
        }
    }
}

impl<T: Serialize + DeserializeOwned + Default> LongLatTree<T> {
    pub fn new(bbox: BoundingBox<i32>, storage_path: PathBuf) -> Self {
        LongLatTree {
            root_tree_info: Rc::new(RootTreeInfo(storage_path)),
            bbox,
            direction: LongLatSplitDirection::default(),
            children: BTreeSet::new(),
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
                        current_tree_children: self.children.iter(),
                    };
                }
            }
            None => {
                return LongLatTreeItems {
                    query_bbox,
                    parent_tree_stack: Vec::with_capacity(0),
                    current_tree_children: self.children.iter(),
                }
            }
        }
    }

    pub fn insert(&mut self, bbox: BoundingBox<i32>, item: T) {
        match &mut self.left_right_split {
            Some((left, right)) => {
                if left.deref().bbox.contains(&bbox) {
                    left.deref_mut().insert(bbox, item);
                    return;
                } else if right.deref().bbox.contains(&bbox) {
                    right.deref_mut().insert(bbox, item);
                    return;
                } else {
                    self.children
                        .insert(tree_child_of(bbox, item, self.direction));
                }
            }
            None => {
                if self.children.len() < NODE_SATURATION_POINT {
                    self.children
                        .insert(tree_child_of(bbox, item, self.direction));
                } else {
                    self.split_left_right();
                    return self.insert(bbox, item);
                }
            }
        }
    }

    fn split_left_right(&mut self) {
        debug_assert!(self.left_right_split.is_none());

        let (left_bb, right_bb) = self.bbox.split_on_axis(&self.direction);

        let new_direction = !self.direction;

        let mut left_children = BTreeSet::new();
        let mut both_children = BTreeSet::new();

        let right_children = self.children.split_off(
            //safety: we only compare by the bounding box; the item is completely irrelevant.
            &tree_child_of(right_bb, Default::default(), self.direction),
        );

        while let Some(BoundingBoxOrderedByXOrY(bbox, direction, item)) = self.children.pop_first()
        {
            if left_bb.contains(&bbox) {
                left_children.insert(tree_child_of(bbox, item, new_direction));
            } else {
                both_children.insert(BoundingBoxOrderedByXOrY(bbox, self.direction, item));
            }
        }

        self.children.append(&mut both_children);
        drop(both_children);

        let (left_path, left_id) = self.make_branch_id(0);
        let (right_path, right_id) = self.make_branch_id(1);

        self.left_right_split = Some((
            Storage::new(
                left_path,
                LongLatTree {
                    root_tree_info: Rc::clone(&self.root_tree_info),
                    bbox: left_bb,
                    direction: !self.direction,
                    children: left_children,
                    left_right_split: None,
                    id: left_id,
                },
            ),
            Storage::new(
                right_path,
                LongLatTree {
                    root_tree_info: Rc::clone(&self.root_tree_info),
                    bbox: right_bb,
                    direction: !self.direction,
                    children: right_children,
                    left_right_split: None,
                    id: right_id,
                },
            ),
        ))
    }

    fn make_branch_id(&self, direction: u64) -> (PathBuf, u64) {
        let new_id = (self.id << 1) | direction;

        let info = self.root_tree_info.0.join(format!("{}", new_id));

        return (info, new_id);
    }
}

trait UpdateOnBasisLongLatMove {
    fn update(&mut self, old_basis: BoundingBox<i32>, new_basis: BoundingBox<i32>);
}

impl<T> UpdateOnBasisLongLatMove for T {
    fn update(&mut self, old_basis: BoundingBox<i32>, new_basis: BoundingBox<i32>) {}
}

fn tree_child_of<T>(
    bbox: BoundingBox<i32>,
    item: T,
    direction: LongLatSplitDirection,
) -> BoundingBoxOrderedByXOrY<i32, T> {
    BoundingBoxOrderedByXOrY::<i32, T>(bbox, direction, item)
}
