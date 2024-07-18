use std::{
    collections::{btree_set, BTreeSet},
    mem,
};

use bbox::{BoundingBox, LongLatSplitDirection};
use compare_by::CompareBy;

pub mod bbox;
pub mod compare_by;

const NODE_SATURATION_POINT: usize = 2000;

pub struct LongLatTree<T: UpdateOnBasisLongLatMove> {
    bbox: BoundingBox<i32>,
    direction: LongLatSplitDirection,
    children: BTreeSet<CompareBy<(BoundingBox<i32>, T)>>,
    left_right_split: Option<(Box<LongLatTree<T>>, Box<LongLatTree<T>>)>,
    id: u64,
}

pub struct LongLatTreeItems<'a, T: UpdateOnBasisLongLatMove> {
    query_bbox: &'a BoundingBox<i32>,
    parent_tree_stack: Vec<&'a LongLatTree<T>>,
    current_tree_children: btree_set::Iter<'a, CompareBy<(BoundingBox<i32>, T)>>,
}

impl<'a, T> Iterator for LongLatTreeItems<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(next) = self.current_tree_children.next() {
                if self.query_bbox.contains(&next.0 .0) {
                    return Some(&next.0 .1);
                }
            } else {
                let tree = self.parent_tree_stack.pop()?;
                if let Some((l, r)) = &tree.left_right_split {
                    self.parent_tree_stack.push(&l);
                    self.parent_tree_stack.push(&r);
                }
                self.current_tree_children = tree.children.iter();
            }
        }
    }
}

impl<T> LongLatTree<T> {
    pub fn new(bbox: BoundingBox<i32>) -> Self {
        LongLatTree {
            bbox,
            direction: LongLatSplitDirection::Lat,
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
            Some((l, r)) => {
                if l.bbox.contains(query_bbox) {
                    return l.find_items_in_box(query_bbox);
                } else if r.bbox.contains(query_bbox) {
                    return r.find_items_in_box(query_bbox);
                } else {
                    return LongLatTreeItems {
                        query_bbox,
                        parent_tree_stack: vec![l, r],
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

    pub fn insert_created(
        &mut self,
        bbox: BoundingBox<i32>,
        creator: impl FnOnce(BoundingBox<i32>) -> T,
    ) {
        match &mut self.left_right_split {
            Some((left, right)) => {
                if left.bbox.contains(&bbox) {
                    return left.insert_created(bbox, creator);
                } else if left.bbox.contains(&bbox) {
                    return right.insert_created(bbox, creator);
                } else {
                    self.children
                        .insert(tree_child_of(bbox, creator(self.bbox), self.direction));
                }
            }
            None => {
                if self.children.len() < NODE_SATURATION_POINT {
                    self.children
                        .insert(tree_child_of(bbox, creator(self.bbox), self.direction));
                } else {
                    self.split_left_right();
                    return self.insert(bbox, creator(self.bbox));
                }
            }
        }
    }

    pub fn insert(&mut self, bbox: BoundingBox<i32>, item: T) {
        match &mut self.left_right_split {
            Some((left, right)) => {
                if left.bbox.contains(&bbox) {
                    return left.insert(bbox, item);
                } else if left.bbox.contains(&bbox) {
                    return right.insert(bbox, item);
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

        let right_children = self
            .children
            .split_off(
                //safety: we only compare by the bounding box; the item is completely irrelevant.
                &unsafe { tree_child_of(right_bb, mem::zeroed(), self.direction) }
            );

        while let Some(CompareBy((bbox, item), func)) = self.children.pop_first() {
            if left_bb.contains(&bbox) {
                left_children.insert(tree_child_of(bbox, item, new_direction));
            } else {
                both_children.insert(CompareBy((bbox, item), func));
            }
        }

        self.children.append(&mut both_children);
        drop(both_children);

        self.left_right_split = Some((
            Box::new(LongLatTree {
                bbox: left_bb,
                direction: !self.direction,
                children: left_children,
                left_right_split: None,
                id: (self.id << 1) | 0,
            }),
            Box::new(LongLatTree {
                bbox: right_bb,
                direction: !self.direction,
                children: right_children,
                left_right_split: None,
                id: (self.id << 1) | 1,
            }),
        ))
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
) -> CompareBy<(BoundingBox<i32>, T)> {
    match direction {
        LongLatSplitDirection::Long => {
            CompareBy::with_cmp((bbox, item), |(a, _), (b, _)| a.x().cmp(&b.x()))
        }
        LongLatSplitDirection::Lat => {
            CompareBy::with_cmp((bbox, item), |(a, _), (b, _)| a.y().cmp(&b.y()))
        }
    }
}
