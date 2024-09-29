use std::io::Write;

use crate::serialize_min::{DeserializeFromMinimal, SerializeMinimal};

pub trait MinimalSerdeFast: SerializeMinimal + DeserializeFromMinimal {
    fn fast_minimally_serialize<'a, 's: 'a, W: Write>(
        &'a self,
        write_to: &mut W,
        external_data: <Self as SerializeMinimal>::ExternalData<'s>,
    ) -> std::io::Result<()>;

    fn fast_deserialize_minimal<'a, 'd: 'a, R: std::io::Read>(
        from: &'a mut R,
        external_data: <Self as DeserializeFromMinimal>::ExternalData<'d>,
    ) -> Result<Self, std::io::Error>;
}

#[repr(transparent)]
#[derive(Copy, Clone, Debug, )]
pub struct FastMinSerde<T: MinimalSerdeFast>(pub T);

impl<T: MinimalSerdeFast> std::ops::Deref for FastMinSerde<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: MinimalSerdeFast> std::ops::DerefMut for FastMinSerde<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T: MinimalSerdeFast> SerializeMinimal for FastMinSerde<T> {
    type ExternalData<'s> = <T as SerializeMinimal>::ExternalData<'s>;

    fn minimally_serialize<'a, 's: 'a, W: Write>(
        &'a self,
        write_to: &mut W,
        external_data: Self::ExternalData<'s>,
    ) -> std::io::Result<()> {
        self.0.fast_minimally_serialize(write_to, external_data)
    }
}

impl<T: MinimalSerdeFast> DeserializeFromMinimal for FastMinSerde<T> {
    type ExternalData<'s> = <T as DeserializeFromMinimal>::ExternalData<'s>;
    
    fn deserialize_minimal<'a, 'd: 'a, R: std::io::Read>(
        from: &'a mut R,
        external_data: Self::ExternalData<'d>,
    ) -> Result<Self, std::io::Error> {
        T::fast_deserialize_minimal(from, external_data).map(|x| FastMinSerde(x))
    }
}

impl<T: MinimalSerdeFast> From<T> for FastMinSerde<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}

macro_rules! impl_fast_primitive_serde {
    ($($typ:ident),*) => {
        $(
            impl MinimalSerdeFast for $typ {
                fn fast_minimally_serialize<'a, 's: 'a, W: Write>(
                    &'a self,
                    write_to: &mut W,
                    _external_data: <Self as SerializeMinimal>::ExternalData<'s>,
                ) -> std::io::Result<()> {
                    write_to.write_all(&self.to_be_bytes())
                }
            
                fn fast_deserialize_minimal<'a, 'd: 'a, R: std::io::Read>(
                    from: &'a mut R,
                    _external_data: <Self as DeserializeFromMinimal>::ExternalData<'d>,
                ) -> Result<Self, std::io::Error> {
                    let mut a = [0; std::mem::size_of::<$typ>()];

                    from.read_exact(&mut a)?;

                    Ok($typ::from_be_bytes(a))
                }
            }
        )*
    };
}

impl_fast_primitive_serde!{i8, u8, i16, u16, i32, u32, i64, u64}