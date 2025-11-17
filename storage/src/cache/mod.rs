use std::{
    cmp,
    collections::{BTreeMap, BinaryHeap},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, OnceLock, 
    },
};

use parking_lot::{lock_api::RwLockWriteGuard, RwLock};

use debug_logs::debug_print;

#[derive(Debug)]
pub struct Cache<Key, Value>
where
    Value: ?Sized,
    Key: Ord + Copy,
{
    cache: RwLock<BTreeMap<Key, OnceLock<(usize, Arc<Value>)>>>,
    cached_bytes: AtomicUsize,
    max_bytes: usize,
}

impl<K, V> Cache<K, V>
where
    V: ?Sized,
    K: Ord + Copy,
{
    pub fn new(max_bytes: usize) -> Self {
        Self {
            cache: Default::default(),
            cached_bytes: 0.into(),
            max_bytes,
        }
    }

    /// This only assures something's existence (or nonexistence!) 
    /// at the time that the function is called. Take care that `Cache`
    /// is designed to be very parallel, and as such this may not be  
    /// usable for long unless external locking is applied.
    pub fn exists(&mut self, id: &K) -> bool {
        self.cache.get_mut().contains_key(&id)
    }

    pub fn get_or_insert(&self, id: K, f: impl FnOnce() -> (usize, Arc<V>)) -> Arc<V> {

        debug_print!("Cache::get_or_insert started");

        if let Some(prev) = self
            .cache
            .read()
            .get(&id)
            .and_then(OnceLock::get)
            .map(|x| Arc::clone(&x.1))
        {
            return prev;
        }

        debug_print!("Cache::get_or_insert item doesnt exist in cache");

        //if the item doesn't exist in the cache, then insert it!

        //first, make an empty OnceLock for it.
        let mut cache = self.cache.write();
        cache.entry(id).or_insert(OnceLock::new());
        
        let cache = RwLockWriteGuard::downgrade(cache);

        debug_print!("Cache::get_or_insert write ended");

        //then, fill it.
        let (cache_added_bytes, value) = cache.get(&id).unwrap().get_or_init(|| {
            //this closure can only be active in one thread at once, so no need to worry about multi-thread shenanigains
            f()
        });

        let value = Arc::clone(value);

        debug_print!("Cache::get_or_insert fill ended");

        //slight possibility of race conditions around `cached_bytes` if
        //multiple threads try to insert differently sized
        //values in the same key, but this is only a high-water mark and doesn't need to be exact
        let cached_bytes = self
            .cached_bytes
            .fetch_add(*cache_added_bytes, Ordering::AcqRel);

        debug_print!("Cache::get_or_insert added");

        drop(cache);

        if cached_bytes > self.max_bytes {
            self.evict();
        }

        value
    }

    fn evict(&self) {
        debug_print!("Cache::evict started");

        let mut cache = self.cache.write();

        debug_print!("Cache::evict lock attained");

        let to_rem = BinaryHeap::from_iter(cache.iter().flat_map(|(k, v)| {
            //use Try w/ flat_map to never remove cells which aren't already filled
            let (size, v) = v.get()?;
            //This is thread-safe because we have a write-lock on the cache,
            //and since the Arc count is 1, the only reference is through the cache.
            //once it decreases to 1, it can never increase until we release the
            //write lock at the end of this function.
            if Arc::strong_count(&v) == 1 {
                return Some((*size, cmp::Reverse(*k)));
            } else {
                None
            }
        }));

        debug_print!("Cache::evict to_rem made");

        let mut total_size = self.cached_bytes.load(Ordering::Relaxed);
        let target_size = self.max_bytes / 2;

        for (size, cmp::Reverse(k)) in to_rem {
            drop(cache.remove(&k));
            self.cached_bytes.fetch_sub(size, Ordering::AcqRel);
            total_size -= size;

            if total_size < target_size {
                break;
            }
        }

        debug_print!("Cache::evict ended");
        
        //Ensure that the cache's exclusive lock is dropped _after_ all of the removals happen. This means that
        //no undefined behaviour will come from modifications during a cache's item being dropped OR from a
        //slot being refilled while it is being emtied
        drop(cache);
    }

    pub fn evict_all_possible(&self) {
        let mut cache = self.cache.write();

        let to_rem = Vec::from_iter(cache.iter().flat_map(|(k, v)| {
            let (size, v) = v.get()?;

            //This is thread-safe because we have a write-lock on the cache,
            //and since the Arc count is 1, the only reference is through the cache.
            //once it decreases to 1, it can never increase until we release the
            //write lock at the end of this function.
            if Arc::strong_count(&v) == 1 {
                return Some((*k, *size));
            } else {
                None
            }
        }));

        for (k, size) in to_rem {
            drop(cache.remove(&k));
            self.cached_bytes.fetch_sub(size, Ordering::Relaxed);
        }

        drop(cache);
    }
}
