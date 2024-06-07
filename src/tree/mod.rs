use std::{collections::VecDeque, ops::Index};

const NODE_SATURATION_POINT: usize = 10000;

struct LongLatTree<T: GeometricBounds> {
    bbox: BoundingBox<i64>,
    direction: LongLatSplitDirection,
    children: Vec<T>,
    left_right_split: Option<(Box<LongLatTree<T>>, Box<LongLatTree<T>>)>,
}

pub struct BoundingBox<T> {
    x: T,
    y: T,
    width: T,
    height: T,
}
impl BoundingBox<i64> {
    fn center(&self) -> (i64, i64) {
        return (self.x + self.width / 2, self.y + self.height / 2);
    }
    fn split_on_axis(&self, direction: &LongLatSplitDirection) -> (Self, Self) {
        match direction {
            LongLatSplitDirection::Long => (
                BoundingBox {
                    x: self.x,
                    y: self.y,
                    width: self.width,
                    height: self.height / 2,
                },
                BoundingBox {
                    x: self.x,
                    y: self.y + self.height / 2,
                    width: self.width,
                    height: self.height / 2,
                },
            ),
            LongLatSplitDirection::Lat => (
                BoundingBox {
                    x: self.x,
                    y: self.y,
                    width: self.width / 2,
                    height: self.height,
                },
                BoundingBox {
                    x: self.x + self.width / 2,
                    y: self.y,
                    width: self.width / 2,
                    height: self.height,
                },
            ),
        }
    }

    fn contains(&self, other: &BoundingBox<i64>) -> bool {
        return self.y <= other.y
            && self.x <= other.y
            && self.y + self.height >= other.y + other.height
            && self.x + self.width >= other.x + other.width;
    }
}

pub trait GeometricBounds {
    fn bounding_box(&self) -> BoundingBox<i64>;
}

pub struct LongLatTreeItems<'a, T: GeometricBounds> {
    query_bbox: &'a BoundingBox<i64>,
    parent_tree_stack: Vec<&'a LongLatTree<T>>,
    current_tree_children: std::slice::Iter<'a, T>,
}

impl<'a, T: GeometricBounds> Iterator for LongLatTreeItems<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(next) = self.current_tree_children.next() {
                if self.query_bbox.contains(&next.bounding_box()) {
                    return Some(next);
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

impl<T: GeometricBounds> LongLatTree<T> {
    pub fn find_items_in_box<'a>(
        &'a self,
        query_bbox: &'a BoundingBox<i64>,
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
                        current_tree_children: [].iter(),
                    };
                }
            }
            None => {
                return LongLatTreeItems {
                    query_bbox,
                    parent_tree_stack: vec![self],
                    current_tree_children: [].iter(),
                }
            }
        }
    }
    pub fn insert(&mut self, item: T) {
        let bbox = item.bounding_box();

        self.insert_as_bbox(item, bbox);
    }

    fn insert_as_bbox(&mut self, item: T, bbox: BoundingBox<i64>) {
        match &mut self.left_right_split {
            Some((left, right)) => {
                if left.bbox.contains(&bbox) {
                    return left.insert_as_bbox(item, bbox);
                } else if left.bbox.contains(&bbox) {
                    return right.insert_as_bbox(item, bbox);
                } else {
                    self.children.push(item);
                }
            }
            None => {
                if self.children.len() < NODE_SATURATION_POINT {
                    self.children.push(item);
                } else {
                    self.split_left_right();
                    return self.insert_as_bbox(item, bbox);
                }
            }
        }
    }

    fn split_left_right(&mut self) {
        debug_assert!(self.left_right_split.is_none());

        let (left_bb, right_bb) = self.bbox.split_on_axis(&self.direction);

        self.left_right_split = Some((
            Box::new(LongLatTree {
                bbox: left_bb,
                direction: !self.direction,
                children: Vec::new(),
                left_right_split: None,
            }),
            Box::new(LongLatTree {
                bbox: right_bb,
                direction: !self.direction,
                children: Vec::new(),
                left_right_split: None,
            }),
        ))
    }
}

#[derive(Clone, Copy)]
pub enum LongLatSplitDirection {
    Long,
    Lat,
}

impl std::ops::Not for LongLatSplitDirection {
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            LongLatSplitDirection::Long => LongLatSplitDirection::Lat,
            LongLatSplitDirection::Lat => LongLatSplitDirection::Long,
        }
    }
}
