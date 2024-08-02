use std::ops::RangeInclusive;

use crate::serialize_min::{DeserializeFromMinimal, SerializeMinimal};

impl DeserializeFromMinimal for bool {
    type ExternalData<'d> = ();

    fn deserialize_minimal<'a, 'd: 'a, R: std::io::Read>(
        from: &'a mut R,
        _external_data: Self::ExternalData<'d>,
    ) -> Result<Self, std::io::Error> {
        Ok(u8::deserialize_minimal(from, ())? != 0)
    }
}

impl SerializeMinimal for bool {
    type ExternalData<'s> = ();

    fn minimally_serialize<'a, 's: 'a, W: std::io::Write>(
        &'a self,
        write_to: &mut W,
        _external_data: Self::ExternalData<'s>,
    ) -> std::io::Result<()> {
        (*self as u8).minimally_serialize(write_to, ())
    }
}

impl DeserializeFromMinimal for () {
    type ExternalData<'d> = ();

    fn deserialize_minimal<'a, 'd: 'a, R: std::io::Read>(
        _from: &'a mut R,
        _external_data: Self::ExternalData<'d>,
    ) -> Result<Self, std::io::Error> {
        Ok(())
    }
}

impl SerializeMinimal for () {
    type ExternalData<'s> = ();

    fn minimally_serialize<'a, 's: 'a, W: std::io::Write>(
        &'a self,
        _write_to: &mut W,
        _external_data: Self::ExternalData<'s>,
    ) -> std::io::Result<()> {
        Ok(())
    }
}

macro_rules! impl_float_serialize {
    ( $($typ:tt ($utyp:tt)),* ) => {
$(
impl SerializeMinimal for $typ {
    type ExternalData<'s> = ();

    fn minimally_serialize<'a, 's: 'a, W: std::io::Write>(&'a self, write_to: &mut W, external_data: Self::ExternalData<'s>) -> std::io::Result<()> {
        self.to_bits().minimally_serialize(write_to, external_data)
    }
}

impl DeserializeFromMinimal for $typ {
    type ExternalData<'s> = ();

    fn deserialize_minimal<'a, 'd: 'a, R: std::io::Read>(from: &'a mut R, external_data: Self::ExternalData<'d>) -> Result<Self, std::io::Error> {
        Ok($typ::from_bits($utyp::deserialize_minimal(from, external_data)?))
    }
}
)*

    }
}

impl_float_serialize! { f32(u32), f64(u64) }

impl<T: SerializeMinimal> SerializeMinimal for RangeInclusive<T>
where for<'a> T::ExternalData<'a>: Copy {
    type ExternalData<'d> = T::ExternalData<'d>;
    
    fn minimally_serialize<'a, 's: 'a, W: std::io::Write>(&'a self, write_to: &mut W, external_data: Self::ExternalData<'s>) -> std::io::Result<()> {
        self.start().minimally_serialize(write_to, external_data)?;
        self.end().minimally_serialize(write_to, external_data)?;

        Ok(())
    }
}

impl<T: DeserializeFromMinimal> DeserializeFromMinimal for RangeInclusive<T>
where for<'a> T::ExternalData<'a>: Copy {
    type ExternalData<'d> = T::ExternalData<'d>;
    
    fn deserialize_minimal<'a, 'd: 'a, R: std::io::Read>(from: &'a mut R, external_data: Self::ExternalData<'d>) -> Result<Self, std::io::Error> {
        Ok(
            (T::deserialize_minimal(from, external_data)?)..=(T::deserialize_minimal(from, external_data)?)
        )    
    }
}