use minimal_storage::serialize_min::{DeserializeFromMinimal, SerializeMinimal};
use std::fmt::Debug;

pub trait MultidimensionalParent<const DIMENSION_COUNT: usize>:
    Sized
    + Clone
    + SerializeMinimal<ExternalData<'static> = ()>
    + DeserializeFromMinimal<ExternalData<'static> = ()>
    + Eq
    + Debug
{
    type DimensionEnum: Dimension<DIMENSION_COUNT>;

    fn contains(&self, child: &Self) -> bool;
    fn overlaps(&self, child: &Self) -> bool;
    fn split_evenly_on_dimension(&self, dimension: &Self::DimensionEnum) -> (Self, Self);
}

pub trait MultidimensionalKey<const DIMENSION_COUNT: usize>:
    Sized + 'static + Clone + Copy + Debug
{
    type Parent: MultidimensionalParent<DIMENSION_COUNT>;

    type DeltaFromParent: Ord + Zero + Copy + Clone + Debug;
    type DeltaFromSelfAsChild: SerializeMinimal<ExternalData<'static> = ()>
        + DeserializeFromMinimal<ExternalData<'static> = ()>
        + Zero
        + Debug;

    fn is_contained_in(&self, parent: &Self::Parent) -> bool;

    fn delta_from_parent(&self, parent: &Self::Parent) -> Self::DeltaFromParent;
    fn apply_delta_from_parent(delta: &Self::DeltaFromParent, parent: &Self::Parent) -> Self;

    /// Can be overriden if wished for speed, but must be equivalent
    /// to `Self::apply_delta_from_parent(delta, parent).is_contained_in(parent)`.
    fn delta_from_parent_would_be_contained(delta: &Self::DeltaFromParent, from: &Self::Parent, container: &Self::Parent) -> bool {
        Self::apply_delta_from_parent(delta, from).is_contained_in(container)
    }

    /// Can be overriden if wished for speed, but must be equivalent
    /// to `Self::apply_delta_from_parent(delta, parent).is_contained_in(parent)`.
    fn delta_from_parent_would_overlap(delta: &Self::DeltaFromParent, from: &Self::Parent, container: &Self::Parent) -> bool {
        Self::apply_delta_from_parent(delta, from).is_contained_in(container)
    }

    fn smallest_key_in(parent: &Self::Parent) -> Self;
    fn largest_key_in(parent: &Self::Parent) -> Self;

    fn delta_from_self(
        finl: &Self::DeltaFromParent,
        initil: &Self::DeltaFromParent,
    ) -> Self::DeltaFromSelfAsChild;
    fn apply_delta_from_self(
        delta: &Self::DeltaFromSelfAsChild,
        initial: &Self::DeltaFromParent,
    ) -> Self::DeltaFromParent;
}

pub trait MultidimensionalValue<Key>:
    'static
    + SerializeMinimal<ExternalData<'static> = ()>
    + for<'deserialize> DeserializeFromMinimal<ExternalData<'deserialize> = &'deserialize Key>
    + Clone
    + Debug
{
}

impl<Key, T> MultidimensionalValue<Key> for T where
    T: 'static
        + SerializeMinimal<ExternalData<'static> = ()>
        + for<'deserialize> DeserializeFromMinimal<ExternalData<'deserialize> = &'deserialize Key>
        + Clone
        + Debug
{
}

pub trait Dimension<const NUM: usize>: Copy {
    fn next_axis(&self) -> Self;
    fn from_index(index: usize) -> Self;
    fn arbitrary_first() -> Self;
}

impl Dimension<1> for () {
    fn next_axis(&self) -> Self {
        ()
    }

    fn from_index(_index: usize) -> Self {
        ()
    }
    
    fn arbitrary_first() -> Self {
        ()
    }
}

pub trait Zero {
    fn zero() -> Self;
}

pub trait Average: Sized {
    fn avg(a: &Self, b: &Self) -> Self;
}

pub trait AbsDiff {
    type Diff;
    fn abs_diff(a: &Self, b: &Self) -> Self::Diff;
}

macro_rules! impl_num_traits {
    ($($typ:ident),*) => {
        $(
        impl Average for $typ {
            #[inline]
            fn avg(a: &Self, b: &Self) -> Self {
                *a / 2 + *b / 2
            }
        }
        impl Zero for $typ {
            #[inline]
            fn zero() -> Self {
                0
            }
        }
    )*
    };
}

impl AbsDiff for i32 {
    type Diff = u32;
    #[inline]
    fn abs_diff(a: &Self, b: &Self) -> Self::Diff {
        a.abs_diff(*b)
    }
}

macro_rules! impl_float_num_traits {
    ($($typ:ident),*) => {
        $(
        impl Average for $typ {
            #[inline]
            fn avg(a: &Self, b: &Self) -> Self {
                *a / 2. + *b / 2.
            }
        }
        impl Zero for $typ {
            #[inline]
            fn zero() -> Self {
                0.
            }
        }

        impl AbsDiff for $typ {
            type Diff = $typ;

            #[inline]
            fn abs_diff(a: &Self, b: &Self) -> Self {
                (a - b).abs()
            }
        }
    )*
    };
}

impl_num_traits! {u8, i8, u16, i16, u32, i32, u64, i64, u128, i128}

impl_float_num_traits! {f32, f64}