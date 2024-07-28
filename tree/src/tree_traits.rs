use minimal_storage::serialize_min::{DeserializeFromMinimal, SerializeMinimal};

pub trait MultidimensionalParent<const DIMENSION_COUNT: usize>: Sized + Clone {
    type DimensionEnum: Dimension<DIMENSION_COUNT>;

    fn contains(&self, child: &Self) -> bool;
    fn split_evenly_on_dimension(&self, dimension: &Self::DimensionEnum) -> (Self, Self);
}

pub trait MultidimensionalKey<const DIMENSION_COUNT: usize>:
    Sized + 'static + Clone + Copy
{
    type Parent: MultidimensionalParent<DIMENSION_COUNT>;

    type DeltaFromParent: Ord + Zero + Copy + Clone;
    type DeltaFromSelf: SerializeMinimal<ExternalData<'static> = ()> + DeserializeFromMinimal<ExternalData<'static> = ()> + Zero;

    fn is_contained_in(&self, parent: &Self::Parent) -> bool;

    fn delta_from_parent(&self, parent: &Self::Parent) -> Self::DeltaFromParent;
    fn apply_delta_from_parent(delta: &Self::DeltaFromParent, parent: &Self::Parent) -> Self;

    fn delta_from_self(finl: &Self::DeltaFromParent, initil: &Self::DeltaFromParent) -> Self::DeltaFromSelf;
    fn apply_delta_from_self(delta: &Self::DeltaFromSelf, initial: &Self::DeltaFromParent) -> Self::DeltaFromParent;
}

pub trait MultidimensionalValue<Key>:
    'static
    + SerializeMinimal
    + for<'deserialize> DeserializeFromMinimal<ExternalData<'deserialize> = &'deserialize Key>
    + Clone
{
}

impl<Key, T> MultidimensionalValue<Key> for T where
    T: 'static
        + SerializeMinimal
        + for<'deserialize> DeserializeFromMinimal<ExternalData<'deserialize> = &'deserialize Key>
        + Clone
{
}

pub trait Dimension<const NUM: usize>: Default {
    fn next_axis(&self) -> Self;
    fn from_index(index: usize) -> Self;
}

impl Dimension<1> for () {
    fn next_axis(&self) -> Self {
        ()
    }

    fn from_index(_index: usize) -> Self {
        ()
    }
}


pub trait Zero {
    fn zero() -> Self;
}

pub trait Average: Sized {
    fn avg(a: Self, b: Self) -> Self;
}

macro_rules! impl_num_traits {
    ($($typ:ident),*) => {
        $(
        impl Average for $typ {
            fn avg(a: Self, b: Self) -> Self {
                a / 2 + b / 2
            }
        }
        impl Zero for $typ {
            fn zero() -> Self {
                0
            }
        }
    )*
    };
}

impl_num_traits! {u8, i8, u16, i16, u32, i32, u64, i64, u128, i128}
