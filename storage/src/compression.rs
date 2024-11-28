use crate::serialize_min::{DeserializeFromMinimal, MinimalSerializedSeek, SerializeMinimal};

pub struct Compressed<T: ?Sized>(T);

impl<T: DeserializeFromMinimal> DeserializeFromMinimal for Compressed<T> {
    type ExternalData<'d> = T::ExternalData<'d>;

    fn deserialize_minimal<'a, 'd: 'a, R: std::io::Read>(
        from: &'a mut R,
        external_data: Self::ExternalData<'d>,
    ) -> Result<Self, std::io::Error> {
        T::deserialize_minimal(&mut zstd::Decoder::new(from)?, external_data).map(Compressed)
    }
    
    fn read_past<'a, 'd: 'a, R: std::io::Read>(
        from: &'a mut R,
        external_data: Self::ExternalData<'d>,
    ) -> std::io::Result<()> {
        T::read_past(&mut zstd::Decoder::new(from)?, external_data)
    }
}

impl<T: MinimalSerializedSeek> MinimalSerializedSeek for Compressed<T> {
    fn seek_past<R: std::io::Read>(from: &mut R) -> std::io::Result<()> {
        T::seek_past(&mut zstd::Decoder::new(from)?)
    }
}


impl<T: SerializeMinimal> SerializeMinimal for Compressed<T> {
    type ExternalData<'d> = T::ExternalData<'d>;
    
    fn minimally_serialize<'a, 's: 'a, W: std::io::Write>(
        &'a self,
        write_to: &mut W,
        external_data: Self::ExternalData<'s>,
    ) -> std::io::Result<()> {
        let mut enc = zstd::Encoder::new(write_to, 0)?;
        self.0.minimally_serialize(&mut enc, external_data)?;
        
        enc.finish().map(|_| ())
    }

    
}
