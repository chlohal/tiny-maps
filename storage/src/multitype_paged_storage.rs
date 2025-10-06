use std::{
    any::Any,
    cmp::min,
    io::{self, BufReader, BufWriter, Read, Write},
    marker::PhantomData,
    ops::{Deref, DerefMut},
    sync::{
        atomic::{AtomicBool, AtomicUsize},
        Arc, Mutex,
    },
    thread::panicking,
};

use debug_logs::debug_print;
use parking_lot::lock_api::RawRwLock;
use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::{
    cache::Cache,
    paged_storage::{
        Page, PageId, PageUse, PagedStorage, ALLOWED_CACHE_PHYSICAL_PAGES, PAGE_HEADER_SIZE,
    },
    pooled_storage::Filelike,
    serialize_min::{DeserializeFromMinimal, SerializeMinimal},
};

#[derive(Debug)]
pub struct MultitypePagedStorage<const PAGE_SIZE_K: usize, File: Filelike = std::fs::File> {
    pageuse: Arc<Mutex<PageUse<PAGE_SIZE_K, File>>>,
    cache: Cache<PageId<PAGE_SIZE_K>, dyn Any + Send + Sync>,
}

impl<const K: usize, File: Filelike + 'static> MultitypePagedStorage<K, File> {
    pub fn open(mut file: File) -> Self {
        let lowest_unallocated_id = match file.metadata().unwrap().len() {
            0 => 1, //if it's a blank file, default to 1 as the lowest unallocated ID (0 is reserved)
            _ => {
                file.seek(io::SeekFrom::Start(PAGE_HEADER_SIZE as u64))
                    .unwrap();
                let mut bytes = [0u8; (usize::BITS / 8) as usize];
                debug_assert!(bytes.len() <= PageId::<K>::data_size());
                file.read_exact(&mut bytes).unwrap();

                usize::from_le_bytes(bytes)
            }
        };

        let pageuse = PageUse {
            lowest_unallocated_id,
            freed_pages: Vec::new(),
            file,
        };

        let pageuse = Arc::new(Mutex::new(pageuse));

        Self {
            pageuse,
            cache: Cache::new(ALLOWED_CACHE_PHYSICAL_PAGES * PageId::<K>::byte_size()),
        }
    }

    pub fn single_type_view<
        T: SerializeMinimal<ExternalData<'static> = ()>
            + DeserializeFromMinimal
            + Send
            + Sync
            + 'static,
    >(
        &self,
    ) -> SingleTypeView<K, File, T> {
        SingleTypeView {
            pageuse: Arc::clone(&self.pageuse),
            cache: Cache::new(ALLOWED_CACHE_PHYSICAL_PAGES * PageId::<K>::byte_size()),
        }
    }
}

impl<
        const K: usize,
        File: Filelike + 'static,
        T: SerializeMinimal<ExternalData<'static> = ()>
            + DeserializeFromMinimal
            + Send
            + Sync
            + 'static,
    > StoreByPage<T> for MultitypePagedStorage<K, File>
{
    type PageId = PageId<K>;

    type Page = Page<K, T, File>;

    fn new_page(&self, item: T) -> Self::PageId {
        let id = self.pageuse.lock().unwrap().alloc_new();

        debug_print!("PagedStorage::new_page calling");

        //this set will always `insert`, never `get`,
        //because the ID was just allocated.
        //(even in the case of reallocated pages,
        //    they'll never be added to the pool for reallocation
        //    until they're evicted from the cache)
        self.cache.get_or_insert(id, || {
            debug_print!("PagedStorage::new_page cache get_or_insert cell entered");

            (
                PageId::<K>::byte_size(),
                Arc::new(Page {
                    pageuse: Arc::clone(&self.pageuse),
                    item: RwLock::new(item),
                    dirty: true.into(),
                    freeable: false.into(),
                    component_pages: vec![id],
                }),
            )
        });

        id
    }

    fn get<'a, 'b>(
        &'a self,
        page_id: &Self::PageId,
        deserialize_data: <T as DeserializeFromMinimal>::ExternalData<'b>,
    ) -> Option<Arc<Self::Page>> {
        if !self.pageuse.lock().unwrap().is_valid(page_id) {
            return None;
        }

        let page = self.cache.get_or_insert(*page_id, || {
            let p =
                Arc::new(Page::<_, T, _>::open(&self.pageuse, page_id, deserialize_data).unwrap());
            let b = p.component_pages.len() * PageId::<K>::byte_size();
            (b, p)
        });

        let page = page.downcast().unwrap();
        Some(page)
    }

    fn flush(&self) {
        self.cache.evict_all_possible();
    }
}

pub struct SingleTypeView<
    const K: usize,
    File: Filelike + 'static,
    T: SerializeMinimal<ExternalData<'static> = ()> + DeserializeFromMinimal + Send + Sync + 'static,
> {
    pageuse: Arc<Mutex<PageUse<K, File>>>,
    cache: Cache<PageId<K>, Page<K, T, File>>,
}

impl<
        const K: usize,
        File: Filelike + 'static,
        T: SerializeMinimal<ExternalData<'static> = ()>
            + DeserializeFromMinimal
            + Send
            + Sync
            + 'static,
    > StoreByPage<T> for SingleTypeView<K, File, T>
{
    type PageId = PageId<K>;
    type Page = Page<K, T, File>;

    fn new_page(&self, item: T) -> PageId<K> {
        let id = self.pageuse.lock().unwrap().alloc_new();

        //this set will always `insert`, never `get`,
        //because the ID was just allocated.
        //(even in the case of reallocated pages,
        //    they'll never be added to the pool for reallocation
        //    until they're evicted from the cache)
        self.cache.get_or_insert(id, || {
            debug_print!("PagedStorage::new_page cache get_or_insert cell entered");

            (
                PageId::<K>::byte_size(),
                Arc::new(Page {
                    pageuse: Arc::clone(&self.pageuse),
                    item: RwLock::new(item),
                    dirty: true.into(),
                    freeable: false.into(),
                    component_pages: vec![id],
                }),
            )
        });

        id
    }

    fn get<'a, 'b>(
        &'a self,
        page_id: &PageId<K>,
        deserialize_data: <T as DeserializeFromMinimal>::ExternalData<'b>,
    ) -> Option<Arc<Page<K, T, File>>> {
        if !self.pageuse.lock().unwrap().is_valid(page_id) {
            return None;
        }

        let page = self.cache.get_or_insert(*page_id, || {
            let p =
                Arc::new(Page::<_, T, _>::open(&self.pageuse, page_id, deserialize_data).unwrap());
            let b = p.component_pages.len() * PageId::<K>::byte_size();
            (b, p)
        });

        Some(page)
    }

    fn flush(&self) {
        self.cache.evict_all_possible();
    }
}

pub trait StoragePage<T> {
    fn get_mut(&mut self) -> &mut T;

    fn read<'a>(&'a self) -> impl Deref<Target = T> + 'a;
    fn read_arc(self: &Arc<Self>) -> impl Deref<Target = T>;

    fn write_arc(self: &Arc<Self>) -> impl DerefMut<Target = T>;
    fn write<'a>(&'a self) -> impl DerefMut<Target = T> + 'a;

    ///
    /// Only call if all references to the Page's ID are inaccessible
    /// If this is called, then the underlying data MUST NOT write anything
    /// when serialized. If the underlying data does so, then the page will not
    /// in fact be freed. If the Page's `write()` method (or any similar) are called before
    /// the page is finished with, then the page will not in fact be freed.
    ///
    unsafe fn allow_free(&self);
}

pub trait StoreByPage<Item: SerializeMinimal + DeserializeFromMinimal> {
    type PageId;
    type Page: StoragePage<Item>;

    fn new_page(&self, item: Item) -> Self::PageId;
    fn get<'a, 'b>(
        &'a self,
        page_id: &Self::PageId,
        deserialize_data: <Item as DeserializeFromMinimal>::ExternalData<'b>,
    ) -> Option<Arc<Self::Page>>;
    fn flush(&self);
}
