use std::{fmt::Debug, io::Write, marker::PhantomData};

use literal_value::LiteralValue;
use sha2::{Digest, Sha256};
use structured_elements::{address::OsmAddress, contact::OsmContactInfo};

use crate::storage::serialize_min::{DeserializeFromMinimal, SerializeMinimal};

use super::{topn::TopNHeap, varint::ToVarint};

pub type LiteralId = u64;

pub mod literal_value;
pub mod packed_strings;
pub mod string_prefix_view;
pub mod structured_elements;

const INLINING_THRESHOLD_BYTES: usize = 3;

pub struct LiteralPool<T: SerializeMinimal> {
    value_count: usize,
    destination: Box<dyn Write>,
    recent_writes: TopNHeap<300, [u8; 32], usize>,
    __phantom: PhantomData<T>,
}

impl<T: SerializeMinimal> LiteralPool<T> {
    pub fn new(destination: Box<dyn Write>) -> Self {
        LiteralPool {
            destination,
            value_count: 0,
            recent_writes: TopNHeap::new(),
            __phantom: PhantomData,
        }
    }

    pub fn flush(&mut self) -> std::io::Result<()> {
        self.destination.flush()
    }
}

impl LiteralPool<Literal> {
    pub fn insert(
        pools: &mut (LiteralPool<Literal>, LiteralPool<LiteralValue>),
        item: &Literal,
    ) -> std::io::Result<LiteralId> {
        let mut blob = Vec::new();
        item.minimally_serialize(&mut blob, pools)?;

        insert_type_irrelevant(&mut pools.0, blob)
    }
}

impl LiteralPool<LiteralValue> {
    pub fn insert(&mut self, item: &LiteralValue) -> std::io::Result<LiteralId> {
        let mut blob = Vec::new();
        item.minimally_serialize(&mut blob, ())?;

        insert_type_irrelevant(self, blob)
    }
}

fn insert_type_irrelevant<T: SerializeMinimal>(
    pool: &mut LiteralPool<T>,
    value_blob: Vec<u8>,
) -> std::io::Result<LiteralId> {
    //let tiny blobs be inline instead of adding them to the pool
    if value_blob.len() <= INLINING_THRESHOLD_BYTES {
        let mut bytes: [u8; 8] = Default::default();

        bytes[8 - value_blob.len()..].copy_from_slice(&value_blob[..]);

        let value = u64::from_be_bytes(bytes);

        debug_assert!(value < u64::MAX);

        return Ok(value << 1);
    }

    let hash_arr = Sha256::digest(&value_blob);
    let hash = hash_arr[..].try_into().unwrap();

    let i = pool
        .recent_writes
        .get(hash)
        .copied()
        .map(|x| Ok::<_, std::io::Error>(x))
        .unwrap_or_else(|| {
            let i: usize = pool.value_count + 1;
            pool.destination.write(&value_blob)?;
            pool.value_count += 1;
            Ok(i)
        })?;

    pool.recent_writes.insert_and_increase(*hash, i);
    return Ok((i as u64) << 1);
}

#[derive(Clone)]
pub enum Literal {
    KeyVar(LiteralKey, LiteralValue),
    WellKnownKeyVar(WellKnownKeyVar),

    Ref(usize),
}

impl<A: Into<LiteralKey>, B: Into<LiteralValue>> From<(A, B)> for Literal {
    fn from(value: (A, B)) -> Self {
        Literal::KeyVar(value.0.into(), value.1.into())
    }
}

impl From<OsmContactInfo> for Literal {
    fn from(value: OsmContactInfo) -> Self {
        Literal::WellKnownKeyVar(WellKnownKeyVar::Contact(value))
    }
}

impl SerializeMinimal for Literal {
    type ExternalData<'a> = &'a mut (LiteralPool<Literal>, LiteralPool<LiteralValue>);

    fn minimally_serialize<'a, 's: 'a, W: Write>(
        &'a self,
        write_to: &mut W,
        pool: Self::ExternalData<'s>,
    ) -> std::io::Result<()> {
        let mut head = 0b0000_0000u8;

        match self {
            Literal::KeyVar(k, v) => {
                head |= 0b0 << 7;

                match k {
                    LiteralKey::WellKnownKey(wkk) => {
                        debug_assert!((*wkk as u8) & 0b1100_0000 == 0);

                        head |= *wkk as u8;
                        write_to.write_all(&[head])?;
                    }
                    LiteralKey::Str(s) => {
                        head |= 1 << 6;
                        write_to.write_all(&[head])?;

                        let id = pool.1.insert(&s.clone().into())?;
                        id.write_varint(write_to)?;
                    }
                }

                let id = pool.1.insert(v)?;
                return id.write_varint(write_to);
            }
            Literal::WellKnownKeyVar(wk) => {
                head |= 0b1 << 7;
                match wk {
                    WellKnownKeyVar::Address(addr) => {
                        head |= 0b00_0000;

                        write_to.write_all(&[head])?;

                        return addr.minimally_serialize(write_to, pool);
                    }
                    WellKnownKeyVar::Contact(contact) => {
                        head |= 0b00_0001;

                        write_to.write_all(&[head])?;

                        return contact.minimally_serialize(write_to, pool);
                    }
                    WellKnownKeyVar::MapFeatureType => {
                        head |= 0b00_0010;

                        write_to.write_all(&[head])?;

                        todo!()
                    }
                }
            }
            Literal::Ref(r) => panic!("Unable to serialize a reference!"),
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum LiteralKey {
    WellKnownKey(WellKnownKey),
    Str(String),
}

impl<R: AsRef<str>> From<R> for LiteralKey {
    fn from(value: R) -> Self {
        Self::Str(value.as_ref().to_string())
    }
}

#[derive(Clone)]
pub enum WellKnownKeyVar {
    Address(OsmAddress),
    MapFeatureType,
    Contact(OsmContactInfo),
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum WellKnownKey {
    Waterway = 0,
}
