use std::{
    fmt::Debug,
    ops::{Add, Div},
};

use postgres::fallible_iterator::{FallibleIterator, FromFallibleIterator};
use serde::{Deserialize, Serialize};

use crate::{
    compressor::varint::{from_varint, FromVarint, ToVarint},
    storage::serialize_min::{DeserializeFromMinimal, ReadExtReadOne, SerializeMinimal},
};

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

impl<T: PartialEq> BoundingBox<T> {
    fn is_point(&self) -> bool {
        self.x == self.x_end && self.y == self.y_end
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

impl<T: ToVarint + PartialEq + Copy> SerializeMinimal for BoundingBox<T> {
    type ExternalData<'s> = ();

    fn minimally_serialize<'a, 's: 'a, W: std::io::Write>(
        &'a self,
        write_to: &mut W,
        _external_data: (),
    ) -> std::io::Result<()> {
        if self.is_point() {
            write_to.write_all(&[0])?;

            self.x.write_varint(write_to)?;
            self.y.write_varint(write_to)?;

            Ok(())
        } else {
            write_to.write_all(&[1])?;

            self.x.write_varint(write_to)?;
            self.y.write_varint(write_to)?;
            self.x_end.write_varint(write_to)?;
            self.y_end.write_varint(write_to)?;

            Ok(())
        }
    }
}

impl<T: FromVarint + Copy> DeserializeFromMinimal for BoundingBox<T> {
    type ExternalData<'a> = ();

    fn deserialize_minimal<'a, 'd: 'a, R: std::io::Read>(
        from: &'a mut R,
        _external_data: (),
    ) -> Result<Self, std::io::Error> {
        let flag = from.read_one()?;

        if flag == 1 {
            let x = from_varint(from)?;
            let y = from_varint(from)?;
            let x_end = from_varint(from)?;
            let y_end = from_varint(from)?;

            Ok(Self { x, y, x_end, y_end })
        }
        //just 2 (point)
        else {
            let x = from_varint(from)?;
            let y = from_varint(from)?;

            Ok(Self {
                x,
                y,
                x_end: x,
                y_end: y,
            })
        }
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
            }
        }
    }

    pub fn interior_delta(&self, parent: &Self) -> DeltaBoundingBox<u32> {
        let x = self.x.abs_diff(parent.x);
        let y = self.y.abs_diff(parent.y);

        let width = self.x.abs_diff(self.x_end);
        let height = self.y.abs_diff(self.y_end);

        DeltaBoundingBox {
            x,
            y,
            width,
            height,
        }
    }

    #[inline]
    pub fn contains(&self, other: &BoundingBox<i32>) -> bool {
        (self.y <= other.y)
            & (self.x <= other.x)
            & (self.x_end >= other.x_end)
            & (self.y_end >= other.y_end)
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

#[derive(Debug, Clone)]
pub struct DeltaBoundingBox<T: lindel::IdealKey<2>>
where
    <T as lindel::IdealKey<2>>::Key: Debug + Clone,
{
    x: T,
    y: T,
    width: T,
    height: T,
}

impl<T: lindel::IdealKey<2> + Default> DeltaBoundingBox<T>
where
    <T as lindel::IdealKey<2>>::Key: Debug + Clone + Default,
{
    pub fn zero() -> DeltaBoundingBox<T> {
        Self {
            x: T::default(),
            y: T::default(),
            width: T::default(),
            height: T::default(),
        }
    }
}

impl DeltaBoundingBox<u32> {
    pub fn morton_origin_point(&self) -> u64 {
        lindel::hilbert_encode([self.x, self.y])
    }

    pub fn delta_friendly_offset(&self, initial: &Self) -> DeltaFriendlyU32Offset {
        DeltaFriendlyU32Offset(
            self.morton_origin_point() - initial.morton_origin_point(),
            self.width,
            self.height,
        )
    }

    pub fn from_delta_friendly_offset(from: &DeltaFriendlyU32Offset, initial: &DeltaFriendlyU32Offset) -> Self {
        let [x, y] = lindel::hilbert_decode(from.0 + initial.0);
        Self {
            x, y,
            width: from.1,
            height: from.2,
        }
    }

    pub fn absolute(&self, parent: &BoundingBox<i32>) -> BoundingBox<i32> {
        let x = parent.y.checked_add_unsigned(self.x).unwrap();
        let y = parent.y.checked_add_unsigned(self.y).unwrap();
        BoundingBox {
            x,
            y,
            x_end: x.checked_add_unsigned(self.width).unwrap(),
            y_end: y.checked_add_unsigned(self.height).unwrap(),
        }
    }
}

pub struct DeltaFriendlyU32Offset(u64, u32, u32);

impl DeltaFriendlyU32Offset {
    pub fn zero() -> Self {
        Self(0,0,0)
    }
}

impl SerializeMinimal for DeltaFriendlyU32Offset {
    type ExternalData<'s> = ();

    fn minimally_serialize<'a, 's: 'a, W: std::io::Write>(
        &'a self,
        write_to: &mut W,
        _external_data: Self::ExternalData<'s>,
    ) -> std::io::Result<()> {
        let DeltaFriendlyU32Offset(mortoned, width, height) = self;
        let is_not_point = *width != 0 || *height != 0;

        if mortoned & (1 << 63) != 0 {
            panic!("Huge mortoned coordinate; no extra bit to encode point-ness");
        }

        let header = (mortoned << 1) | (is_not_point as u64);

        header.write_varint(write_to)?;

        if is_not_point {
            width.minimally_serialize(write_to, ())?;
            height.minimally_serialize(write_to, ())?;
        }

        Ok(())
    }
}

impl DeserializeFromMinimal for DeltaFriendlyU32Offset {
    type ExternalData<'a> = ();

    fn deserialize_minimal<'a, 'd: 'a, R: std::io::Read>(
        from: &'a mut R,
        external_data: Self::ExternalData<'d>,
    ) -> Result<Self, std::io::Error> {
        let header = u64::deserialize_minimal(from, external_data)?;

        let xy = header >> 1;

        let is_not_point = (header & 1) != 0;

        let (width, height) = if is_not_point {
            (
                u32::deserialize_minimal(from, ())?,
                u32::deserialize_minimal(from, ())?,
            )
        } else {
            (0, 0)
        };

        Ok(Self( xy, width, height ))
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn contains() {
        let big = BoundingBox {
            x: 0,
            y: -900000000,
            x_end: 1800000000,
            y_end: 900000000,
        };
        let small = BoundingBox {
            x: 323422752,
            y: -1,
            x_end: 323422752,
            y_end: -1,
        };
        assert!(big.contains(&small));
    }
}
