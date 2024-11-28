use std::{
    borrow::Cow,
    io::{Read, Seek, Write},
    marker::PhantomData,
};

use crate::serialize_min::{DeserializeFromMinimal, MinimalSerializedSeek, SerializeMinimal};
use lru_cache::TopNHeap;
use sha2::{Digest, Sha256};

const INLINING_AS_ID_THRESHOLD_BYTES: usize = 4;

pub type PooledId = u64;

pub trait Filelike: Write + Seek + Read {}
impl<T: Write + Seek + Read> Filelike for T {}

const BLOCK_WRITE: usize = 3000;
const BLOCK_HEADER_SIZE: u64 = 8;

pub struct Pool<T> {
    value_count: usize,

    pool_offset: u64,
    current_block_first_value_byte: u64,
    current_block_first_value_index: usize,
    block_value_count: usize,
    current_block_size_bytes: u64,
    destination: Box<dyn Filelike>,

    recent_writes: TopNHeap<BLOCK_WRITE, [u8; 32], PooledId>,
    recent_reads: TopNHeap<BLOCK_WRITE, usize, T>,
    __phantom: PhantomData<T>,
}

impl<T: DeserializeFromMinimal + MinimalSerializedSeek + Clone> Pool<T> {
    pub fn get(
        &mut self,
        id: PooledId,
        external_data: T::ExternalData<'_>,
    ) -> std::io::Result<Option<Cow<T>>> {
        //if it's inlined, return the owned data serialized into the ID
        let (idx, external_data) = match Self::id_to_maybe_item(id, external_data) {
            Ok(f) => return Ok(Some(Cow::Owned(f?))),
            Err(e) => e,
        };

        //if the index is too high, return none
        if idx >= self.value_count {
            return Ok(None);
        }

        //if it's in the cache, return a borrow from the cache
        if self.recent_reads.contains(&idx) {
            return Ok(self.recent_reads.get(&idx).map(|x| Cow::Borrowed(x)));
        }

        let (block, index_in_block) = (idx / BLOCK_WRITE, idx % BLOCK_WRITE);

        //then: seek to the current block
        let is_in_current_block = idx >= self.current_block_first_value_index
            && (idx - self.current_block_first_value_index) < BLOCK_WRITE;

        if is_in_current_block {
            self.destination.seek(std::io::SeekFrom::Start(
                self.current_block_first_value_byte,
            ))?;
        } else {
            let block_count = idx / BLOCK_WRITE;
            self.destination
                .seek(std::io::SeekFrom::Start(self.pool_offset))?;
            for _ in 0..block_count {
                let mut h = [0u8; size_of::<u64>()];
                self.destination.read_exact(&mut h)?;

                let byte_count = u64::from_le_bytes(h);

                self.destination
                    .seek_relative(byte_count as i64 + BLOCK_HEADER_SIZE as i64)?;
            }

            self.destination.seek_relative(BLOCK_HEADER_SIZE as i64)?;
        }

        //read past every previous item
        let index_in_block = idx % BLOCK_WRITE;
        for _ in 0..index_in_block {
            T::seek_past(&mut self.destination)?;
        }

        //and read the item (finally)
        let val = T::deserialize_minimal(&mut self.destination, external_data)?;
        //put it in the cache
        self.recent_reads.insert_and_increase(idx, val);

        //and return a borrow from the cache
        Ok(self.recent_reads.get(&idx).map(|x| Cow::Borrowed(x)))
    }

    fn id_to_maybe_item(
        id: PooledId,
        external_data: T::ExternalData<'_>,
    ) -> Result<std::io::Result<T>, (usize, T::ExternalData<'_>)> {
        //if LSB is unset, then it's inlined
        if (id & 1) == 0 {
            let blob = id.to_le_bytes();
            return Ok(T::deserialize_minimal(&mut &blob[..], external_data));
        }

        //else, shift it right to give the actual index
        let index = (id >> 1) as usize;

        Err((index, external_data))
    }
}

impl<T: SerializeMinimal> Pool<T> {
    pub fn new(mut destination: Box<dyn Filelike>) -> std::io::Result<Self> {
        let pool_offset = destination.stream_position()?;
        destination.write_all(&[0; 8])?;

        Ok(Pool {
            destination,
            value_count: 0,
            recent_writes: TopNHeap::new(),
            recent_reads: TopNHeap::new(),
            __phantom: PhantomData,

            pool_offset,
            current_block_size_bytes: 0,
            block_value_count: 0,
            current_block_first_value_index: 0,
            current_block_first_value_byte: pool_offset + BLOCK_HEADER_SIZE,
        })
    }

    pub fn flush(&mut self) -> std::io::Result<()> {
        self.destination.flush()
    }

    fn post_insert(&mut self, blob: &Vec<u8>) -> std::io::Result<()> {
        self.current_block_size_bytes += blob.len() as u64;
        self.block_value_count += 1;

        if self.block_value_count >= BLOCK_WRITE {
            dbg!("writing block header");
            //seek back to the header, then through the header to its start

            dbg!(&self.destination.stream_position());
            dbg!(self.current_block_size_bytes + BLOCK_HEADER_SIZE);
            dbg!(blob.len());

            let current_offset_from_block = 0 - ((self.current_block_size_bytes + BLOCK_HEADER_SIZE) as i64);
            self.destination.seek(std::io::SeekFrom::Current(current_offset_from_block)).unwrap();

            /*self.destination
                .seek_relative()
                .unwrap();
            */

            //write the byte count into the header
            self.destination
                .write_all(&self.current_block_size_bytes.to_le_bytes())
                .unwrap();

            //seek back to the end of the current block
            self.destination
                .seek_relative(self.current_block_size_bytes as i64)
                .unwrap();

            //write the next value's header
            self.destination.write_all(&[0; 8]).unwrap();
            //and reset bookkeeping values
            self.current_block_first_value_index += self.block_value_count;
            self.current_block_size_bytes = 0;
            self.block_value_count = 0;
        }

        Ok(())
    }

    pub fn insert<'s>(
        &mut self,
        item: &'s T,
        ctx: T::ExternalData<'s>,
    ) -> std::io::Result<PooledId> {
        let mut blob = Vec::new();
        item.minimally_serialize(&mut blob, ctx).unwrap();

        let r = self.insert_blob(&blob).unwrap();

        Ok(r)
    }

    fn insert_blob(&mut self, value_blob: &Vec<u8>) -> std::io::Result<PooledId> {
        //let tiny blobs be inline instead of adding them to the pool
        if value_blob.len() <= INLINING_AS_ID_THRESHOLD_BYTES {
            let mut bytes: [u8; 8] = Default::default();

            bytes[..value_blob.len()].copy_from_slice(&value_blob[..]);

            let value = u64::from_le_bytes(bytes);

            debug_assert!(value < u64::MAX);

            //LSB is 0 to indicate that this is inlined
            return Ok(value << 1);
        }

        let hash_arr = Sha256::digest(&value_blob);
        let hash = hash_arr[..].try_into().unwrap();

        if let Some(id) = self.recent_writes.get(hash).copied() {
            return Ok(id);
        }


        let value_index = self.value_count;
        self.destination.write_all(&value_blob)?;
        self.value_count += 1;

        let id = as_noninlined_id(value_index);


        self.recent_writes.insert_and_increase(*hash, id);

        self.post_insert(&value_blob)?;

        //LSB is 1 to indicate that this is not inlined
        return Ok(id);
    }
}

fn as_noninlined_id(i: usize) -> PooledId {
    ((i as u64) << 1) + 1
}
