use std::io::{self, Read, Write};

pub trait SerializeMinimal {
    type ExternalData<'s>;

    fn minimally_serialize<'a, 's: 'a, W: Write>(
        &'a self,
        write_to: &mut W,
        external_data: Self::ExternalData<'s>,
    ) -> std::io::Result<()>;
}

pub trait DeserializeFromMinimal: Sized {
    type ExternalData<'d>;

    fn deserialize_minimal<'a, 'd: 'a, R: Read>(
        from: &'a mut R,
        external_data: Self::ExternalData<'d>,
    ) -> Result<Self, std::io::Error>;

    fn read_past<'a, 'd: 'a, R: Read>(
        from: &'a mut R,
        external_data: Self::ExternalData<'d>,
    ) -> std::io::Result<()> {
        Self::deserialize_minimal(from, external_data).map(|_|())
    }
}

pub trait MinimalSerializedSeek: DeserializeFromMinimal {
    fn seek_past<R: Read>(from: &mut R) -> std::io::Result<()>;
}

pub trait ReadExtReadOne: Read {
    fn read_one(&mut self) -> std::io::Result<u8>;
    fn reading_iterator<'a>(&'a mut self) -> ReadingIterator<'a, Self>;
}

pub fn assert_serialize_roundtrip<
    'a,
    T: PartialEq + std::fmt::Debug + SerializeMinimal + DeserializeFromMinimal,
>(
    item: T,
    ser: <T as SerializeMinimal>::ExternalData<'a>,
    der: <T as DeserializeFromMinimal>::ExternalData<'a>,
) {
    let mut buf = Vec::new();

    item.minimally_serialize(&mut buf, ser).unwrap();

    eprintln!("{} bytes", buf.len());

    let mut stderr = io::stderr().lock();
    for byte in buf.iter() {
        write!(stderr, "{:02x}", byte).unwrap();
    }
    writeln!(stderr, "").unwrap();
    drop(stderr);

    let item_roundtrip = T::deserialize_minimal(&mut &buf[..], der).unwrap();

    assert_eq!(item, item_roundtrip);
}

pub struct ReadingIterator<'a, R: Read + ?Sized>(&'a mut R);

impl<'a, R: Read + ?Sized> Iterator for ReadingIterator<'a, R> {
    type Item = std::io::Result<u8>;

    fn next(&mut self) -> Option<Self::Item> {
        let r = self.0.read_one();

        match r {
            Ok(_) => Some(r),
            Err(e) => match e.kind() {
                std::io::ErrorKind::UnexpectedEof => None,
                _ => Some(Err(e)),
            },
        }
    }
}

impl<T: Read + ?Sized> ReadExtReadOne for T {
    fn read_one(&mut self) -> Result<u8, std::io::Error> {
        let r = &mut [0u8];

        loop {
            match self.read(r) {
                Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
                Ok(_) => return Ok(r[0]),
                Err(e) => if e.kind() != io::ErrorKind::Interrupted { return Err(e) },
            }
        }
    }

    fn reading_iterator<'a>(&'a mut self) -> ReadingIterator<'a, Self> {
        ReadingIterator(self)
    }
}
