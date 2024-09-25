use std::{cmp, collections::{BTreeMap, BinaryHeap}, sync::{atomic::{AtomicUsize, Ordering}, Arc, RwLock}};

pub trait SizeEstimate {
    fn estimated_bytes(&self) -> usize;
}

#[derive(Debug)]
pub struct Cache<Key, Value> 
where
    Value: SizeEstimate,
    Key: Ord + Copy
{
    cache: Arc<RwLock<BTreeMap<Key, (usize, Arc<Value>)>>>,
    cached_bytes: AtomicUsize,
    max_bytes: usize
}

impl<K, V> Cache<K, V>
where
    V: SizeEstimate,
    K: Ord + Copy
{
    pub fn new(max_bytes: usize) -> Self {
        Self {
            cache: Default::default(),
            cached_bytes: 0.into(),
            max_bytes,
        }
    }
    pub fn get(&self, id: &K) -> Option<Arc<V>> {
        let cache = self.cache.read().unwrap();

        cache.get(id).map(|x| Arc::clone(&x.1))
    }

    pub fn insert(&self, id: K, value: V) -> Arc<V> {
        let mut cache = self.cache.write().unwrap();

        let value_size = value.estimated_bytes();

        let value = Arc::new(value);

        let old = cache.insert(id, (value_size, Arc::clone(&value)));

        drop(cache);

        let cached_bytes = if let Some((old_size, old)) = old {
            drop(old);

            self.cached_bytes.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |v| {
                Some(v.saturating_sub(old_size) + value_size)
            }).unwrap()
        } else {
            self.cached_bytes.fetch_add(value_size, Ordering::AcqRel)
        };

        if cached_bytes > self.max_bytes {
            self.evict();
        }

        value
    }

    fn evict(&self) {
        let mut cache = self.cache.write().unwrap();

        let mut to_rem = BinaryHeap::new();
        for (k, (size, v)) in cache.iter() {
            //This is thread-safe because we have a write-lock on the cache,
            //and since the Arc count is 1, the only reference is through the cache.
            //once it decreases to 1, it can never increase until we release the 
            //write lock at the end of this function.
            if Arc::strong_count(&v) == 1 {
                to_rem.push((*size, cmp::Reverse(*k)));
            }
        }

        let mut total_size = self.cached_bytes.load(Ordering::Relaxed);
        let prev_total_size = total_size;

        for (size, cmp::Reverse(k)) in to_rem {
            drop(cache.remove(&k));
            self.cached_bytes.fetch_sub(size, Ordering::AcqRel);
            total_size -= size;

            if total_size < self.max_bytes {
                break;
            }
        }

        drop(cache);
    }
    
    pub fn evict_all_possible(&self) {
        let mut cache = self.cache.write().unwrap();

        let mut to_rem = Vec::new();
        for (k, (size, v)) in cache.iter() {
            //This is thread-safe because we have a write-lock on the cache,
            //and since the Arc count is 1, the only reference is through the cache.
            //once it decreases to 1, it can never increase until we release the 
            //write lock at the end of this function.
            if Arc::strong_count(&v) == 1 {
                to_rem.push((*k, *size));
            }
        }

        for (k, size) in to_rem {
            drop(cache.remove(&k));
            self.cached_bytes.fetch_sub(size, Ordering::Relaxed);
        }
    }
}