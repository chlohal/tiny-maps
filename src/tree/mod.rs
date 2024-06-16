use std::{collections::VecDeque, ops::Index};

use postgres::fallible_iterator::{FallibleIterator, FromFallibleIterator};

const NODE_SATURATION_POINT: usize = 10000;

pub struct LongLatTree<T: GeometricBounds> {
    bbox: BoundingBox<i64>,
    direction: LongLatSplitDirection,
    children: Vec<T>,
    left_right_split: Option<(Box<LongLatTree<T>>, Box<LongLatTree<T>>)>,
    id: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct BoundingBox<T> {
    x: T,
    y: T,
    width: T,
    height: T,
}

impl FromFallibleIterator<(i64, i64)> for BoundingBox<i64> {
    fn from_fallible_iter<I>(it: I) -> Result<Self, I::Error>
    where
        I: postgres::fallible_iterator::IntoFallibleIterator<Item = (i64, i64)>,
    {
        let mut iter = it.into_fallible_iter();

        let mut bbox = if let Some((x, y)) = iter.next()? {
            BoundingBox::from_point(x, y)
        } else {
            return Ok(BoundingBox::empty());
        };

        iter.for_each(|(x, y)| {
            bbox.extend_with_point(x.into(), y.into());
            Ok(())
        })?;

        Ok(bbox)
    }
}

impl FromIterator<(i64, i64)> for BoundingBox<i64> {
    fn from_iter<I: IntoIterator<Item = (i64, i64)>>(iter: I) -> Self {
        let mut iter = iter.into_iter();

        let mut bbox = if let Some((x, y)) = iter.next() {
            BoundingBox::from_point(x, y)
        } else {
            return BoundingBox::empty();
        };

        for (x, y) in iter {
            bbox.extend_with_point(x.into(), y.into());
        }

        bbox
    }
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

    pub fn empty() -> BoundingBox<i64> {
        BoundingBox {
            x: i64::MIN,
            y: i64::MIN,
            width: 0,
            height: 0,
        }
    }

    pub fn from_point(x: i64, y: i64) -> BoundingBox<i64> {
        BoundingBox {
            x,
            y,
            width: 0,
            height: 0,
        }
    }

    fn extend_with_point(&mut self, x: i64, y: i64) {
        if self.x > x {
            self.x = x;
        }
        if self.y > y {
            self.y = y;
        }

        if self.x + self.width < x {
            self.width = self.x.abs_diff(x) as i64;
        }

        if self.y + self.height < y {
            self.height = self.y.abs_diff(y) as i64;
        }
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
    pub fn new() -> Self {
        LongLatTree {
            bbox: BoundingBox {
                x: i64::MIN / 2,
                y: i64::MIN / 2,
                width: i64::MAX,
                height: i64::MAX,
            },
            direction: LongLatSplitDirection::Lat,
            children: Vec::new(),
            left_right_split: None,
            id: 1,
        }
    }
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

    pub fn insert_as_bbox(&mut self, item: T, bbox: BoundingBox<i64>) {
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

        let mut left_children = Vec::with_capacity(NODE_SATURATION_POINT / 2);
        let mut right_children = Vec::with_capacity(NODE_SATURATION_POINT / 2);
        let mut both_children = Vec::new();

        for child in self.children.drain(..).into_iter() {
            if left_bb.contains(&child.bounding_box()) {
                left_children.push(child);
            } else if right_bb.contains(&child.bounding_box()) {
                right_children.push(child);
            } else {
                both_children.push(child);
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
                id: self.id << 1 | 0,
            }),
            Box::new(LongLatTree {
                bbox: right_bb,
                direction: !self.direction,
                children: right_children,
                left_right_split: None,
                id: self.id << 1 | 1,
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
