use std::ops::{Add, Div};

use postgres::fallible_iterator::{FallibleIterator, FromFallibleIterator};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct BoundingBox<T> {
    x: T,
    y: T,
    x_end: T,
    y_end: T,
}

pub const EARTH_BBOX: BoundingBox<i32> =
    BoundingBox::new(-1800000000, -900000000, 1800000000, 900000000);

impl FromFallibleIterator<(i32, i32)> for BoundingBox<i32> {
    fn from_fallible_iter<I>(it: I) -> Result<Self, I::Error>
    where
        I: postgres::fallible_iterator::IntoFallibleIterator<Item = (i32, i32)>,
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

impl FromIterator<(i32, i32)> for BoundingBox<i32> {
    fn from_iter<I: IntoIterator<Item = (i32, i32)>>(iter: I) -> Self {
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

impl<T> BoundingBox<T> {
    pub const fn new(x: T, y: T, x_end: T, y_end: T) -> Self {
        Self { x, y, x_end, y_end }
    }
    pub const fn x(&self) -> &T {
        &self.x
    }

    pub const fn y(&self) -> &T {
        &self.y
    }
}

fn avg<T: Add<Output = T> + Div<i32, Output = T>>(a: T, b: T) -> T {
    a / 2 + b / 2
}

impl<T: Copy + Add<Output = T> + Div<i32, Output = T>> BoundingBox<T> {
    pub fn center(&self) -> (T, T) {
        return (avg(self.x, self.x_end), avg(self.y, self.y_end));
    }
}

impl BoundingBox<i32> {
    pub fn split_on_axis(&self, direction: &LongLatSplitDirection) -> (Self, Self) {
        match direction {
            LongLatSplitDirection::Long => {
                let y_split = avg(self.y, self.y_end);
                return (
                    BoundingBox {
                        y_end: y_split,
                        ..*self
                    },
                    BoundingBox {
                        y: y_split,
                        ..*self
                    },
                );
            }
            LongLatSplitDirection::Lat => {
                let x_split = avg(self.x, self.x_end);

                return (
                    BoundingBox {
                        x_end: x_split,
                        ..*self
                    },
                    BoundingBox {
                        x: x_split,
                        ..*self
                    },
                );
            },
        }
    }

    pub fn contains(&self, other: &BoundingBox<i32>) -> bool {
        return self.y <= other.y
            && self.x <= other.y
            && self.x_end >= other.x_end
            && self.y_end >= other.y_end;
    }

    pub fn empty() -> BoundingBox<i32> {
        BoundingBox {
            x: 0,
            y: 0,
            y_end: 0,
            x_end: 0,
        }
    }

    pub fn from_point(x: i32, y: i32) -> BoundingBox<i32> {
        BoundingBox {
            x,
            y,
            x_end: x,
            y_end: y,
        }
    }

    fn extend_with_point(&mut self, x: i32, y: i32) {
        if self.x > x {
            self.x = x;
        }
        if self.y > y {
            self.y = y;
        }

        if self.y_end < y {
            self.y_end = y;
        }

        if self.x_end < x {
            self.x_end = x;
        }
    }
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum LongLatSplitDirection {
    Long,
    Lat,
}

impl Default for LongLatSplitDirection {
    fn default() -> Self {
        LongLatSplitDirection::Lat
    }
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
