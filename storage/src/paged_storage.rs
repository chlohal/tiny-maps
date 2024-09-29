use std::{
    cmp::min, fs::File, io::{self, BufReader, BufWriter, Read, Seek, Write}, ops::{Deref, DerefMut}, path::PathBuf, sync::{atomic::AtomicBool, Arc, Mutex, RwLock}
};

const THOUSAND: usize = 1024;
const PAGE_HEADER_SIZE: usize = 16;

const ALLOWED_CACHE_PHYSICAL_PAGES: usize = 3000;

use crate::cache::{Cache, SizeEstimate};

use super::serialize_min::{DeserializeFromMinimal, SerializeMinimal};

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Copy)]
pub struct PageId<const PAGE_SIZE: usize>(usize);

impl<const K: usize> DeserializeFromMinimal for PageId<K> {
    type ExternalData<'d> = (); 

    fn deserialize_minimal<'a, 'd: 'a, R: Read>(
        from: &'a mut R,
        _external_data: Self::ExternalData<'d>,
    ) -> Result<Self, std::io::Error> {
        let mut bytes = [0u8; (usize::BITS / 8) as usize];
        debug_assert!(bytes.len() <= 8);

        from.read_exact(&mut bytes)?;

        let inner = usize::from_le_bytes(bytes);

        Ok(PageId(inner))
    }
}

impl<const K: usize> SerializeMinimal for PageId<K> {
    type ExternalData<'d> = ();

    fn minimally_serialize<'a, 's: 'a, W: Write>(
        &'a self,
        write_to: &mut W,
        _external_data: Self::ExternalData<'s>,
    ) -> std::io::Result<()> {
        let inner = (self.0 as usize).to_le_bytes();
        write_to.write_all(&inner)
    }
}

impl<const K: usize> PageId<K> {

    #[cfg(debug_assertions)]
    pub fn new(inner: usize) -> Self {
        Self(inner)
    } 
    
    fn byte_offset(&self) -> u64 {
        (self.0 * K * THOUSAND) as u64
    }

    fn data_byte_offset(&self) -> u64 {
        (self.0 * K * THOUSAND + PAGE_HEADER_SIZE) as u64
    }

    fn end_byte_offset(&self) -> u64 {
        ((self.0 + 1) * K * THOUSAND) as u64
    }

    fn as_valid(self) -> Option<PageId<K>> {
        match self.0 {
            0 => None,
            i => Some(Self(i)),
        }
    }

    #[inline]
    const fn data_size() -> usize {
        (K * THOUSAND) - PAGE_HEADER_SIZE
    }

    #[inline]
    const fn byte_size() -> usize {
        K * THOUSAND
    }
}

#[derive(Debug)]
pub struct PagedStorage<const PAGE_SIZE_K: usize, T>
where
    T: SerializeMinimal<ExternalData<'static> = ()> + DeserializeFromMinimal,
{
    pageuse: Arc<Mutex<PageUse<PAGE_SIZE_K>>>,
    cache: Cache<PageId<PAGE_SIZE_K>, Page<PAGE_SIZE_K, T>>
}

#[derive(Debug)]
struct PageUse<const PAGE_SIZE_K: usize>
{
    lowest_unallocated_id: usize,
    freed_pages: Vec<PageId<PAGE_SIZE_K>>,
    file: File,
}

impl<const K: usize> PageUse<K> {
    pub fn alloc_new(&mut self) -> PageId<K> {
        if let Some(p) = self.freed_pages.pop() {
            return p;
        }

        let file = &mut self.file;

        let new_id_num = self.lowest_unallocated_id;
        self.lowest_unallocated_id += 1;
        debug_assert_ne!(new_id_num, usize::MAX);
        let id = PageId(new_id_num);

        let newer_lowest_unallocated_id = new_id_num + 1;

        

        file
            .seek(io::SeekFrom::Start(PAGE_HEADER_SIZE as u64))
            .unwrap();
        file
            .write_all(&newer_lowest_unallocated_id.to_le_bytes())
            .unwrap();

        if file.metadata().unwrap().len() < id.end_byte_offset() {
            file.set_len(id.end_byte_offset()).unwrap();
        }
        file
            .seek(io::SeekFrom::Start(id.byte_offset()))
            .unwrap();
        file.write_all(&[0; PAGE_HEADER_SIZE]).unwrap();

        id
    }

    pub fn alloc_new_after(&mut self, old_page: PageId<K>) -> PageId<K> {
        let new_page = self.alloc_new();

        let file = &mut self.file;

        file
            .seek(io::SeekFrom::Start(old_page.byte_offset()))
            .unwrap();
        new_page.minimally_serialize(&mut *file, ()).unwrap();

        file
            .seek(io::SeekFrom::Start(new_page.byte_offset() + 8))
            .unwrap();
        old_page.minimally_serialize(&mut *file, ()).unwrap();

        new_page
    }

    pub fn free_page(&mut self, free: PageId<K>) {
        let previous_page = {
            self.file
                .seek(io::SeekFrom::Start(free.byte_offset() + 8))
                .unwrap();
            PageId::<K>::deserialize_minimal(&mut self.file, ())
                .unwrap()
                .as_valid()
        };

        self.freed_pages.push(free);

        if let Some(prev) = previous_page {
            self.file
                .seek(io::SeekFrom::Start(prev.byte_offset()))
                .unwrap();
            self.file.write_all(&vec![0; 8]).unwrap();
        }

        self.file
            .seek(io::SeekFrom::Start(free.byte_offset()))
            .unwrap();
        self.file.write_all(&[0; PAGE_HEADER_SIZE]).unwrap();
    }

    pub fn is_valid(&self, id: &PageId<K>) -> bool {
        self.lowest_unallocated_id > id.0
    }
}

impl<const K: usize, T> PagedStorage<K, T>
where
    T: SerializeMinimal<ExternalData<'static> = ()> + DeserializeFromMinimal,
{
    pub fn open(id: PathBuf) -> Self {
        let mut file = open_file(&id);

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
            cache: Cache::new(ALLOWED_CACHE_PHYSICAL_PAGES * PageId::<K>::byte_size())
        }
    }

    pub fn new_page(&self, item: T) -> PageId<K> {
        let id = self.pageuse.lock().unwrap().alloc_new();

        let page = Page {
            pageuse: Arc::clone(&self.pageuse),
            item: RwLock::new(item),
            dirty: true.into(),
            component_pages: vec![id],
        };
        
        self.cache.insert(id, page);

        id
    }

    pub fn get<'a, 'b>(
        &'a self,
        page_id: &PageId<K>,
        deserialize_data: <T as DeserializeFromMinimal>::ExternalData<'b>,
    ) -> Option<Arc<Page<K, T>>> {
        if !self.pageuse.lock().unwrap().is_valid(page_id) {
            return None;
        }

        let cached = self.cache.get(page_id);

        if let Some(cached) = cached {
            return Some(cached);
        }

        let page = Page::open(&self.pageuse, page_id, deserialize_data).unwrap();

        Some(self.cache.insert(*page_id, page))
    }
    
    pub fn flush(&self) {
        self.cache.evict_all_possible();
    }
}

fn open_file(path: &PathBuf) -> File {
    File::options()
        .create(true)
        .read(true)
        .write(true)
        .open(&path)
        .unwrap()
}

#[derive(Debug)]
pub struct Page<const PAGE_SIZE_K: usize, T>
where
    T: SerializeMinimal<ExternalData<'static> = ()> + DeserializeFromMinimal,
{
    item: RwLock<T>,
    dirty: AtomicBool,
    component_pages: Vec<PageId<PAGE_SIZE_K>>,

    pageuse: Arc<Mutex<PageUse<PAGE_SIZE_K>>>
}


impl<const K: usize, T> SizeEstimate for Page<K, T>
where
    T: SerializeMinimal<ExternalData<'static> = ()> + DeserializeFromMinimal
    {
        fn estimated_bytes(&self) -> usize {
        self.component_pages.len() * PageId::<K>::byte_size()
    }
    }

impl<const K: usize, T> Page<K, T>
where
    T: SerializeMinimal<ExternalData<'static> = ()> + DeserializeFromMinimal,
{
    pub fn read<'a>(&'a self) -> impl Deref<Target = T> + 'a {
        self.item.read().unwrap()
    }

    pub fn write<'a>(&'a self) -> impl DerefMut<Target = T> + 'a {
        self.dirty.store(true, std::sync::atomic::Ordering::Relaxed);

        self.item.write().unwrap()
    }

    fn open<'a>(
        pageuse: &Arc<Mutex<PageUse<K>>>,
        page_id: &PageId<K>,
        deserialize: <T as DeserializeFromMinimal>::ExternalData<'a>,
    ) -> std::io::Result<Self> {
        let pg = &mut pageuse.lock().unwrap();

        let mut reader = BufReader::with_capacity(PageId::<K>::data_size(), PageReader::<K, true> {
            file: &mut pg.file,
            page_ids_acc: vec![*page_id],
            current_page_id: *page_id,
            current_page_read_amount: 0,
        });

        let item = T::deserialize_minimal(&mut reader, deserialize)?;

        let reader = reader.into_inner();

        Ok(Self {
            pageuse: Arc::clone(pageuse),
            item: RwLock::new(item),
            dirty: false.into(),
            component_pages: reader.page_ids_acc,
        })
    }

    fn flush<'a>(&mut self) -> std::io::Result<()> {

        let mut storage = self.pageuse.lock().unwrap();

        if self.dirty.load(std::sync::atomic::Ordering::Relaxed) {
            let mut writer = BufWriter::new(PageWriter {
                storage: storage.deref_mut(),
                added_pages: vec![],
                state: WriterState::Begin {
                    to_write: &self.component_pages,
                },
            });

            self.item.get_mut().unwrap().minimally_serialize(&mut writer, ())?;
            writer.flush()?;

            let writer = writer
                .into_inner()
                .map_err(|_| Into::<std::io::Error>::into(std::io::ErrorKind::BrokenPipe))
                .expect("no error when flushing buffer");

            let PageWriter {
                added_pages, state, ..
            } = writer;

            let freed_pages = match state {
                WriterState::Begin { to_write }
                | WriterState::WritingAllocated { to_write, .. } => {
                    Vec::from(to_write)
                },
                _ => vec![],
            };

            debug_assert!(freed_pages.is_empty() || added_pages.is_empty());

            let valid_written_len = self.component_pages.len() - freed_pages.len();
            self.component_pages.truncate(valid_written_len);

            self.component_pages.extend_from_slice(&added_pages);

            for page_to_free in freed_pages {
                storage.free_page(page_to_free);
            }
        }

        Ok(())
    }
}

impl<const K: usize, T> Drop for Page<K, T>
where
    T: SerializeMinimal<ExternalData<'static> = ()> + DeserializeFromMinimal,
{
    fn drop(&mut self) {
        if let Err(e) = self.flush() {
            eprintln!("{}", e);
        }
    }
}

pub(super) fn read_page_header<const K: usize>(
    file: &mut File,
    page_id: &PageId<K>,
) -> std::io::Result<Option<PageId<K>>> {
    let pos_bytes = page_id.byte_offset();

    file.seek(io::SeekFrom::Start(pos_bytes))?;
    let next_page = PageId::deserialize_minimal(file, ())?;

    Ok(next_page.as_valid())
}

#[derive(Debug)]
enum WriterState<'a, const PAGE_SIZE_K: usize> {
    Begin {
        to_write: &'a [PageId<PAGE_SIZE_K>],
    },
    WritingAllocated {
        written: usize,
        current: PageId<PAGE_SIZE_K>,
        to_write: &'a [PageId<PAGE_SIZE_K>],
    },
    WritingNew {
        written: usize,
        current: PageId<PAGE_SIZE_K>,
    },
    NeedsNewAllocation {
        previous: PageId<PAGE_SIZE_K>,
    },
}

impl<'a, const K: usize> WriterState<'a, K> {
    pub fn increase_data_offset(&mut self, addend: usize) {
        match self {
            WriterState::Begin { .. } => {
                panic!("WriterState::Begin does not have a data offset to increase!")
            }
            WriterState::NeedsNewAllocation { .. } => {
                panic!("WriterState::NeedsNewAllocation does not have a data offset to increase!")
            }

            WriterState::WritingAllocated { written, .. } => *written += addend,
            WriterState::WritingNew { written, .. } => *written += addend,
        }
    }
}

#[derive(Debug)]
struct PageWriter<'a, const PAGE_SIZE_K: usize>
{
    storage: &'a mut PageUse<PAGE_SIZE_K>,
    added_pages: Vec<PageId<PAGE_SIZE_K>>,
    state: WriterState<'a, PAGE_SIZE_K>,
}

impl<'a, const K: usize> Write for PageWriter<'a, K>
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let (current_page, current_data_offset) = match self.state {
            WriterState::Begin { mut to_write } => {
                let current = *slice_pop(&mut to_write).unwrap();
                self.state = WriterState::WritingAllocated {
                    written: 0,
                    current,
                    to_write,
                };
                (current, 0)
            }
            WriterState::WritingAllocated {
                current, written, ..
            } => (current, written),
            WriterState::WritingNew {
                current, written, ..
            } => (current, written),
            WriterState::NeedsNewAllocation { previous } => {
                let current = self.storage.alloc_new_after(previous);
                self.added_pages.push(current);
                self.state = WriterState::WritingNew {
                    written: 0,
                    current,
                };
                (current, 0)
            }
        };

        let remaining_bytes = PageId::<K>::data_size() - current_data_offset;

        let trimmed_buf = &buf[0..min(buf.len(), remaining_bytes)];

        self.storage.file.seek(io::SeekFrom::Start(
            current_page.data_byte_offset() + (current_data_offset as u64),
        ))?;
        let write_amnt = self.storage.file.write(trimmed_buf)?;

        if write_amnt == remaining_bytes {
            match self.state {
                WriterState::WritingAllocated {
                    current, to_write, ..
                } => {
                    if to_write.len() == 0 {
                        self.state = WriterState::NeedsNewAllocation { previous: current };
                    } else {
                        self.state = WriterState::Begin { to_write };
                    }
                }
                WriterState::WritingNew { current, .. } => {
                    self.state = WriterState::NeedsNewAllocation { previous: current }
                }
                _ => unreachable!(),
            }
        } else {
            self.state.increase_data_offset(write_amnt)
        }

        Ok(write_amnt)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.storage.file.flush()
    }
}

fn slice_pop<'a, 'b, T>(slice: &'a mut &'b [T]) -> Option<&'b T> {
    let head = slice.get(0)?;

    *slice = &slice[1..];

    Some(head)
}

struct PageReader<'a, const PAGE_SIZE_K: usize, const BUILD_COMPONENT_ID_LIST: bool> {
    file: &'a mut File,
    page_ids_acc: Vec<PageId<PAGE_SIZE_K>>,
    current_page_id: PageId<PAGE_SIZE_K>,
    current_page_read_amount: usize,
}

impl<'a, const K: usize, const B: bool> Read for PageReader<'a, K, B> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let available_space = PageId::<K>::data_size() - self.current_page_read_amount;

        let buf_len = buf.len();
        let buffer_trimmed = &mut buf[0..min(buf_len, available_space)];

        self.file.seek(io::SeekFrom::Start(
            self.current_page_id.data_byte_offset() + (self.current_page_read_amount as u64),
        ))?;

        let read_count = self.file.read(buffer_trimmed)?;

        self.current_page_read_amount += read_count;

        if self.current_page_read_amount == PageId::<K>::data_size() {
            let next_page_id = read_page_header(&mut self.file, &self.current_page_id)?;

            if let Some(next_page_id) = next_page_id {
                if B {
                    self.page_ids_acc.push(next_page_id);
                }

                self.current_page_id = next_page_id;
                self.current_page_read_amount = 0;
            }
        }

        Ok(read_count)
    }
}

#[cfg(test)]
mod test {
    use lru_cache::TopNHeap;

    use super::*;

    mod store_many_pages {
        use super::*;

        #[test]
        pub fn store_many_pages() {
            let growth_rate = 2;
            let growth_max = 100;
            let blob_count = 2000;

            let _ = std::fs::remove_file(".test");
            let mut storage = PagedStorage::<4, Vec<usize>>::open(".test".into());
            let mut ids = Vec::new();

            //initial population
            for i in 0..blob_count {
                ids.push(storage.new_page(vec![i]));
            }

            let mut size = 1;

            //grow each page exponentially until we reach the maximum length
            while size <= growth_max {
                let old_size = size;

                size *= growth_rate;

                for (i, id) in ids.iter().enumerate() {
                    let item = storage.get(id, ()).unwrap();

                    validate_blob(&item.read(), i, old_size);

                    item.write().resize(size, i);
                }
                eprintln!("{size}");
            }

            //shrink each page exponentially until we get back to 1
            while size > 1 {
                let old_size = size;

                size /= growth_rate;

                for (i, id) in ids.iter().enumerate() {
                    let item = storage.get(id, ()).unwrap();

                    validate_blob(&item.read(), i, old_size);

                    item.write().resize(size, i);
                }
                eprintln!("{size}");
            }
        }

        fn validate_blob(item: &Vec<usize>, fill: usize, size: usize) {
            assert_eq!(item.len(), size);
            assert!(item.iter().all(|x| *x == fill));
        }
    }

    #[test]
    pub fn cache_testing() {
        let mut cache = TopNHeap::<20, PageId<4>, bool>::new();

        cache.insert_and_increase(PageId(1), true);

        let pos = cache.get_index(&PageId(1)).unwrap();

        assert_eq!(*cache.index(pos).unwrap(), true);
    }

    #[test]
    pub fn store_one_page() {
        let _ = std::fs::remove_file(".test");
        let mut storage = PagedStorage::<4, _>::open(".test".into());

        let mut ids = Vec::new();

        let blob_len = 4;

        let fill_with = 0x65u32;
        let start = 0x68u32;

        for _ in 0..21 {
            let weird_buf = vec![fill_with; blob_len];
            let id = storage.new_page(weird_buf);

            ids.push(id);
        }

        storage.flush();

        dbg!(&storage);

        for id in ids.iter() {
            let page = storage.get(&id, ()).unwrap();

            page.write()[0] = start;
        }

        storage.flush();

        for id in ids.iter() {
            let page = storage.get(&id, ()).unwrap();

            let mut weird_vec = vec![fill_with; blob_len];
            weird_vec[0] = start;

            assert_eq!(weird_vec, *page.read());
        }

        storage.flush();
    }
}
