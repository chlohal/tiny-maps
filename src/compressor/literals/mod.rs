use std::{array, collections::BTreeMap, marker::PhantomData, ops::Deref};

use literal_value::LiteralValue;
use packed_strings::PackedString;
use radix_trie::Trie;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use structured_elements::address::OsmAddress;

use super::topn::TopNHeap;

pub type LiteralId = u64;

pub mod structured_elements;
pub mod literal_value;
pub mod packed_strings;
pub mod string_prefix_view;


const INLINING_THRESHOLD_BYTES: usize = 3;

#[derive(Serialize, Deserialize)]
pub struct LiteralPool<T: OsmLiteralArchetype> {
    values: Vec<Vec<u8>>,
    #[serde(skip)] 
    recent_writes: TopNHeap<[u8; 32], usize>,
    #[serde(skip)] 
    __phantom: PhantomData<T>,
}

impl<T: OsmLiteralArchetype> LiteralPool<T> {
    pub fn new() -> Self {
        LiteralPool {
            values: vec![],
            recent_writes: TopNHeap::new(200),
            __phantom: PhantomData,
        }
    }
}

impl LiteralPool<Literal> {
    pub fn insert<G: OsmLiteralSerializable<Category = Literal>>(pools: &mut (LiteralPool<Literal>, LiteralPool<LiteralValue>), item: &G) -> LiteralId {
        let value_blob = item.serialize_to_pool(pools);

        insert_type_irrelevant(&mut pools.0, value_blob)
    }
}

impl LiteralPool<LiteralValue> {
    pub fn insert<G: OsmLiteralSerializable<Category = LiteralValue>>(&mut self, item: &G) -> LiteralId {
        let value_blob = item.serialize_to_pool(self);

        insert_type_irrelevant(self, value_blob)
    }
}


fn insert_type_irrelevant<T: OsmLiteralArchetype>(pool: &mut LiteralPool<T>, value_blob: Vec<u8>) -> LiteralId {
    //let tiny blobs be inline instead of adding them to the pool
    if value_blob.len() <= INLINING_THRESHOLD_BYTES {
        let mut bytes: [u8; 8] = Default::default();

        bytes[8-value_blob.len()..].copy_from_slice(&value_blob[..]);

        let value = u64::from_be_bytes(bytes);

        debug_assert!(value < u64::MAX);

        return value << 1;
    }

    let hash_arr = Sha256::digest(&value_blob);
    let hash = hash_arr[..].try_into().unwrap();

    let i = pool.recent_writes.get(hash).copied().unwrap_or_else(|| {
        let i: usize = pool.values.len();
        pool.values.push(value_blob);
        i
    });

    pool.recent_writes.insert_and_increase(*hash, i);
    return (i as u64) << 1;
}

impl OsmLiteralArchetypePool<Literal> for LiteralPool<Literal> {
    fn insert<G: OsmLiteralSerializable<Category = Literal>>(
        insert_to: &mut (LiteralPool<Literal>, LiteralPool<LiteralValue>),
        value: &G,
    ) -> LiteralId {
        let blob = value.serialize_to_pool(insert_to);

        insert_type_irrelevant(&mut insert_to.0, blob)
    }
}

impl OsmLiteralArchetypePool<LiteralValue> for LiteralPool<LiteralValue> {
    fn insert<G: OsmLiteralSerializable<Category = LiteralValue>>(
        insert_to: &mut LiteralPool<LiteralValue>,
        value: &G,
    ) -> LiteralId {
        let blob = value.serialize_to_pool(insert_to);

        insert_type_irrelevant(insert_to, blob)
    }
}

pub trait OsmLiteralArchetypePool<T: OsmLiteralArchetype> {
    fn insert<G: OsmLiteralSerializable<Category = T>>(
        insert_to: &mut T::SerializationReference,
        value: &G,
    ) -> LiteralId;
}

#[derive()]
pub enum Literal {
    KeyVar(LiteralKey, LiteralValue),
    WellKnownKeyVar(WellKnownKeyVar),
    Ref(usize),
}

impl OsmLiteralSerializable for Literal {
    type Category = Literal;

    fn serialize_to_pool(
        &self,
        pool: &mut (LiteralPool<Literal>, LiteralPool<LiteralValue>),
    ) -> Vec<u8> {
        todo!()
    }
}

pub enum LiteralKey {
    WellKnownKey(WellKnownKey),
    Str(PackedString),
}

pub enum WellKnownKeyVar {
    Address(OsmAddress),
    MapFeatureType,
}

trait OsmLiteralPool {
    type ReferenceForSerialization;
}

impl OsmLiteralArchetype for Literal {
    type SerializationReference = (LiteralPool<Literal>, LiteralPool<LiteralValue>);
}

impl OsmLiteralArchetype for LiteralValue {
    type SerializationReference = LiteralPool<LiteralValue>;
}

trait OsmLiteralArchetype {
    type SerializationReference;
}

pub trait OsmLiteralSerializable: Sized {
    type Category: OsmLiteralArchetype;

    fn serialize_to_pool(
        &self,
        pool: &mut <Self::Category as OsmLiteralArchetype>::SerializationReference,
    ) -> Vec<u8>;
}

pub enum WellKnownKey {}
