use std::{io::Write, marker::PhantomData};

use minimal_storage::serialize_min::{DeserializeFromMinimal, SerializeMinimal};
use sha2::{Digest, Sha256};

use crate::{aux::topn::TopNHeap, literal::Literal, literal_value::LiteralValue, INLINING_AS_ID_THRESHOLD_BYTES};

pub type LiteralId = u64;

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

pub fn attempt_literal_value_from_id(id: LiteralId) -> std::io::Result<LiteralValue> {
    if id & 1 == 1 {
        let id = id >> 1;
        let blob = id.to_le_bytes();

        LiteralValue::deserialize_minimal(&mut &blob[..], ())
    } else {
        Ok(LiteralValue::Ref(id >> 1))
    }
}

fn insert_type_irrelevant<T: SerializeMinimal>(
    pool: &mut LiteralPool<T>,
    value_blob: Vec<u8>,
) -> std::io::Result<LiteralId> {
    //let tiny blobs be inline instead of adding them to the pool
    if value_blob.len() <= INLINING_AS_ID_THRESHOLD_BYTES {
        let mut bytes: [u8; 8] = Default::default();

        bytes[..value_blob.len()].copy_from_slice(&value_blob[..]);

        let value = u64::from_le_bytes(bytes);

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
