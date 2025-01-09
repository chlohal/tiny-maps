use minimal_storage::{serialize_fast::MinimalSerdeFast, serialize_min::{DeserializeFromMinimal, SerializeMinimal}};

use crate::tree_traits::MultidimensionalKey;

pub mod structure;
pub mod tree;
pub mod tree_serde;

pub trait SparseKey<const DIMENSION_COUNT: usize>:
    MultidimensionalKey<DIMENSION_COUNT>
    + SerializeMinimal<ExternalData<'static> = ()>
    + DeserializeFromMinimal<ExternalData<'static> = ()>
    + Ord
    + MinimalSerdeFast
{
}
impl<
        const DIMENSION_COUNT: usize,
        T: MultidimensionalKey<DIMENSION_COUNT>
            + SerializeMinimal<ExternalData<'static> = ()>
            + DeserializeFromMinimal<ExternalData<'static> = ()>
            + Ord
            + MinimalSerdeFast,
    > SparseKey<DIMENSION_COUNT> for T
{
}

pub trait SparseValue:
    'static
    + MinimalSerdeFast
    + SerializeMinimal<ExternalData<'static> = ()>
    + DeserializeFromMinimal<ExternalData<'static> = ()>
    + Clone
    + std::fmt::Debug
{
}

impl<T> SparseValue for T where
    T: 'static
        + SerializeMinimal<ExternalData<'static> = ()>
        + DeserializeFromMinimal<ExternalData<'static> = ()>
        + Clone
        + std::fmt::Debug
        + MinimalSerdeFast
{
}
