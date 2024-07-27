use std::io::{Write, Read};

use bincode::ErrorKind;


pub trait SerializeMinimal {
    type ExternalData<'s>;

    fn minimally_serialize<'a, 's: 'a, W: Write>(&'a self, write_to: &mut W, external_data: Self::ExternalData<'s>) -> std::io::Result<()>;
}

pub trait DeserializeFromMinimal: Sized {
    type ExternalData<'d>;

    fn deserialize_minimal<'a, 'd: 'a, R: Read>(from: &'a mut R, external_data: Self::ExternalData<'d>) -> Result<Self, std::io::Error>;
}

pub trait ReadExtReadOne: Read {
    fn read_one(&mut self) -> std::io::Result<u8>;
    fn reading_iterator<'a>(&'a mut self) -> ReadingIterator<'a, Self>;
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

        self.read_exact(r)?;

        Ok(r[0])
    }
    
    fn reading_iterator<'a>(&'a mut self) -> ReadingIterator<'a, Self> {
        ReadingIterator(self)
    }
}