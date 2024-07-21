use std::{cmp::Ordering, ops::Deref};

use serde::{Deserialize, Serialize};

use super::bbox::{BoundingBox, LongLatSplitDirection};


#[derive(Deserialize, Serialize)]
pub struct BoundingBoxOrderedByXOrY<T: Ord, I> (pub BoundingBox<T>, pub LongLatSplitDirection, pub I);

impl<T: Ord, I> PartialEq for BoundingBoxOrderedByXOrY<T, I> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(&other).is_eq()
    }
}
impl<T: Ord, I> Eq for BoundingBoxOrderedByXOrY<T, I> {}


impl<T: Ord, I> PartialOrd for BoundingBoxOrderedByXOrY<T, I> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Ord, I> Ord for BoundingBoxOrderedByXOrY<T, I> {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.1 {
            LongLatSplitDirection::Long => self.0.x().cmp(other.0.x()),
            LongLatSplitDirection::Lat => self.0.x().cmp(other.0.x()),
        }
    }
}