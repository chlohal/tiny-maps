use std::{marker::PhantomData, ops::{Add, Sub}};

use crate::{compressor::varint::{FromVarint, ToVarint}, storage::{serialize_min::{DeserializeFromMinimal, SerializeMinimal}, Storage}};

use super::{tree_traits::{Average, MultidimensionalKey, MultidimensionalParent, Zero}, LongLatTree, RootTreeInfo};

pub type StoredBinaryTree<K, T> = Storage<(RootTreeInfo, u64, PointRange<K>), LongLatTree<1, Point<K>, DisregardWhenDeserializing<Point<K>, T>>>;

pub struct DisregardWhenDeserializing<Disregard, T> (T, PhantomData<Disregard>);

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

impl<Disregard: 'static, T: DeserializeFromMinimal<ExternalData<'static> = ()>> DeserializeFromMinimal for DisregardWhenDeserializing<Disregard, T> {
    type ExternalData<'d> = &'d Disregard;

    fn deserialize_minimal<'a, 'd: 'a, R: std::io::Read>(from: &'a mut R, external_data: Self::ExternalData<'d>) -> Result<Self, std::io::Error> {
        Ok(DisregardWhenDeserializing(
            T::deserialize_minimal(from, ())?,
            PhantomData
        ))
    }
}

impl<Disregard: 'static, T: SerializeMinimal> SerializeMinimal for DisregardWhenDeserializing<Disregard, T> {
    type ExternalData<'s> = T::ExternalData<'s>;
    
    fn minimally_serialize<'a, 's: 'a, W: std::io::Write>(&'a self, write_to: &mut W, external_data: Self::ExternalData<'s>) -> std::io::Result<()> {
        self.0.minimally_serialize(write_to, external_data)
    }

    
}

pub trait OneDimensionalCoord: 'static + Average + Clone + Copy + Zero + Ord + ToVarint + FromVarint + Add<Output = Self> + Sub<Output = Self> {}

impl<T: 'static + Average + Clone + Copy + Zero + Ord + ToVarint + FromVarint + Add<Output = T> + Sub<Output = T>> OneDimensionalCoord for T {}

#[derive(Copy, Clone)]
pub struct PointRange<T: OneDimensionalCoord>(pub T, pub T);

impl<T: OneDimensionalCoord> MultidimensionalParent<1> for PointRange<T> {
    type DimensionEnum = ();

    fn contains(&self, child: &Self) -> bool {
        self.0 <= child.0 && child.1 <= self.1
    }

    fn split_evenly_on_dimension(&self, _dimension: &()) -> (Self, Self) {
        let middle = Average::avg(self.0, self.1);

        (PointRange(self.0, middle), PointRange(middle, self.1))
    }
}

#[derive(Copy, Clone)]
pub struct Point<T: OneDimensionalCoord>(pub T);

impl<T: OneDimensionalCoord> MultidimensionalKey<1> for Point<T> {
    type Parent = PointRange<T>;

    type DeltaFromParent = T;

    type DeltaFromSelf = T;

    fn is_contained_in(&self, parent: &Self::Parent) -> bool {
        parent.0 <= self.0 && self.0 <= parent.1
    }

    fn delta_from_parent(&self, parent: &Self::Parent) -> Self::DeltaFromParent {
        self.0 - parent.0
    }

    fn apply_delta_from_parent(delta: &Self::DeltaFromParent, parent: &Self::Parent) -> Self {
        Self(*delta + parent.0)
    }

    fn delta_from_self(finl: &Self::DeltaFromParent, initil: &Self::DeltaFromParent) -> Self::DeltaFromSelf {
        *finl - *initil
    }

    fn apply_delta_from_self(delta: &Self::DeltaFromSelf, initial: &Self::DeltaFromParent) -> Self::DeltaFromParent {
        *initial + *delta
    }
}

