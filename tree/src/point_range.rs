use std::{
    marker::PhantomData,
    ops::{Add, RangeInclusive, Sub},
};

use minimal_storage::{
    serialize_min::{DeserializeFromMinimal, SerializeMinimal},
    varint::{FromVarint, ToVarint},
};

use crate::structure::LongLatTree;

use super::tree_traits::{Average, MultidimensionalKey, MultidimensionalParent, Zero};

pub type StoredBinaryTree<K, T> = LongLatTree<1, K, DisregardWhenDeserializing<K, T>>;

pub struct DisregardWhenDeserializing<Disregard, T>(T, PhantomData<Disregard>);

impl<Disregard, T: std::fmt::Debug> std::fmt::Debug for DisregardWhenDeserializing<Disregard, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl<Disregard, T> DisregardWhenDeserializing<Disregard, T> {
    pub fn into_inner(self) -> T {
        self.0
    }
    pub fn inner(&self) -> &T {
        &self.0
    }
}

impl<Disregard, T: Clone> Clone for DisregardWhenDeserializing<Disregard, T> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), PhantomData)
    }
}

impl<Disregard, T> From<T> for DisregardWhenDeserializing<Disregard, T> {
    fn from(value: T) -> Self {
        DisregardWhenDeserializing(value, PhantomData)
    }
}

impl<Disregard, T: Copy> Copy for DisregardWhenDeserializing<Disregard, T> {}

impl<Disregard: 'static, T: DeserializeFromMinimal<ExternalData<'static> = ()>>
    DeserializeFromMinimal for DisregardWhenDeserializing<Disregard, T>
{
    type ExternalData<'d> = &'d Disregard;

    fn deserialize_minimal<'a, 'd: 'a, R: std::io::Read>(
        from: &'a mut R,
        _external_data: Self::ExternalData<'d>,
    ) -> Result<Self, std::io::Error> {
        Ok(DisregardWhenDeserializing(
            T::deserialize_minimal(from, ())?,
            PhantomData,
        ))
    }
}

impl<Disregard: 'static, T: SerializeMinimal> SerializeMinimal
    for DisregardWhenDeserializing<Disregard, T>
{
    type ExternalData<'s> = T::ExternalData<'s>;

    fn minimally_serialize<'a, 's: 'a, W: std::io::Write>(
        &'a self,
        write_to: &mut W,
        external_data: Self::ExternalData<'s>,
    ) -> std::io::Result<()> {
        self.0.minimally_serialize(write_to, external_data)
    }
}

pub trait OneDimensionalCoord:
    'static
    + Average
    + Clone
    + Copy
    + Zero
    + Ord
    + ToVarint
    + FromVarint
    + Add<Output = Self>
    + Sub<Output = Self>
    + std::fmt::Debug
{
}

impl<
        T: 'static
            + Average
            + Clone
            + Copy
            + Zero
            + Ord
            + ToVarint
            + FromVarint
            + Add<Output = T>
            + Sub<Output = T>
            + std::fmt::Debug,
    > OneDimensionalCoord for T
{
}

impl<T: OneDimensionalCoord> MultidimensionalParent<1> for RangeInclusive<T> {
    type DimensionEnum = ();

    fn contains(&self, child: &Self) -> bool {
        self.start() <= child.start() && child.end() <= self.end()
    }

    fn split_evenly_on_dimension(&self, _dimension: &()) -> (Self, Self) {
        let middle = Average::avg(*self.start(), *self.end());

        (
            (*self.start())..=middle,
            middle..=(*self.end())
        )
    }
}

impl<T: OneDimensionalCoord> MultidimensionalKey<1> for T {
    type Parent = RangeInclusive<T>;

    type DeltaFromParent = T;

    type DeltaFromSelf = T;

    fn is_contained_in(&self, parent: &Self::Parent) -> bool {
        parent.start() <= self && self <= parent.end()
    }

    fn delta_from_parent(&self, parent: &Self::Parent) -> Self::DeltaFromParent {
        *self - *parent.start()
    }

    fn apply_delta_from_parent(delta: &Self::DeltaFromParent, parent: &Self::Parent) -> Self {
        *delta + *parent.start()
    }

    fn delta_from_self(
        finl: &Self::DeltaFromParent,
        initil: &Self::DeltaFromParent,
    ) -> Self::DeltaFromSelf {
        *finl - *initil
    }

    fn apply_delta_from_self(
        delta: &Self::DeltaFromSelf,
        initial: &Self::DeltaFromParent,
    ) -> Self::DeltaFromParent {
        *initial + *delta
    }
}
