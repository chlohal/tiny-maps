use std::{
    borrow::Borrow,
    cell::Cell,
    fs::{remove_file, File},
    io::{self, Seek, Write},
    ops::{Deref, DerefMut},
    os::fd::AsFd,
    path::PathBuf,
};

enum Seq<T> {
    InteriorMutable(Cell<Option<Box<T>>>),
    Safe(Box<T>),
}

use lazy_file::LazyFile;
use serialize_min::{DeserializeFromMinimal, SerializeMinimal};
use Seq::*;

const JUMBLE_COLLECTOR_MAX_GEN: usize = 250;

pub struct Storage<D, T>
where
    T: SerializeMinimal,
    for<'a> T: DeserializeFromMinimal<ExternalData<'a> = &'a D>,
{
    inner: Seq<T>,
    dirty: bool,
    file: LazyFile,
    deserialize_data: D,
    jumble_collector_generation: usize,
}

pub mod serialize_min;
mod lazy_file;

pub trait StorageReachable<DeserializationData>: SerializeMinimal + for<'a> DeserializeFromMinimal<ExternalData<'a> = &'a DeserializationData> {
    
    fn flush_children<'a>(&'a mut self, _serialize_data: <Self as SerializeMinimal>::ExternalData<'a>) -> std::io::Result<()> {
        Ok(())
    }
}

impl<D, T> Storage<D, T>
where
    for<'a> <T as SerializeMinimal>::ExternalData<'a>: Copy,
    T: 'static + StorageReachable<D> + SerializeMinimal,
    for<'a> T: DeserializeFromMinimal<ExternalData<'a> = &'a D>,
{
    pub fn new<'a>(
        id: PathBuf,
        value: T,
        deserialize_data: D,
    ) -> Self {
        let file = File::options()
            .create(true)
            .read(true)
            .write(true)
            .open(&id)
            .unwrap();

        Self {
            inner: Seq::new(value),
            file: LazyFile::new(id),
            dirty: false,
            deserialize_data,
            jumble_collector_generation: 0,
        }
    }

    pub fn open(
        id: PathBuf,
        deserialize_data: D,
    ) -> Self {
        let file = File::options()
            .create(false)
            .read(true)
            .write(true)
            .open(&id)
            .unwrap();

        Self {
            inner: Seq::empty(),
            file: LazyFile::new(id),
            dirty: true,
            deserialize_data,
            jumble_collector_generation: 0,
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    fn increase_and_check_jumble_collector(&mut self) {
        self.jumble_collector_generation += 1;

        if self.jumble_collector_generation >= JUMBLE_COLLECTOR_MAX_GEN {
            self.inner.free();
        }
    }

    pub fn flush<'a>(
        &'a mut self,
        serialize_data: <T as SerializeMinimal>::ExternalData<'a>,
    ) -> Option<Result<(), io::Error>> {
        if !self.dirty {
            self.increase_and_check_jumble_collector();
            return Some(Ok(()));
        }

        match self.flush_without_children_no_dirty_check(serialize_data) {
            Some(Err(e)) => return Some(Err(e)),
            Some(_) | None => {}
        }

        match self.inner.as_mut()?.flush_children(serialize_data) {
            Ok(_) => Some(Ok(())),
            Err(e) => return Some(Err(e)),
        }
    }

    pub fn flush_without_children<'a, 's: 'a>(
        &'a mut self,
        serialize_data: <T as SerializeMinimal>::ExternalData<'s>,
    ) -> Option<Result<(), io::Error>> {
        if self.dirty {
            return self.flush_without_children_no_dirty_check(serialize_data);
        } else {
            self.increase_and_check_jumble_collector();
            return Some(Ok(()));
        }
    }

    fn flush_without_children_no_dirty_check<'a, 's: 'a>(
        &'a mut self,
        serialize_data: <T as SerializeMinimal>::ExternalData<'s>,
    ) -> Option<Result<(), io::Error>> {
        let value = self.inner.as_ref()?;

        let mut file_clone = self.file.try_clone().unwrap();

        //this whole struct is non-sync, so we can do this fearlessly
        file_clone.rewind().unwrap();
        file_clone.set_len(0).unwrap();

        //avoid many small allocations by serializing to a buffer first
        //this does the same thing as BufWriter, but it's easier & does the same
        //for performance
        let mut buf = Vec::new();

        value.minimally_serialize(&mut buf, serialize_data).unwrap();

        let e = file_clone.write_all(&buf);

        match file_clone.flush() {
            Ok(_) => {}
            Err(e) => return Some(Err(e)),
        }

        match e {
            Ok(()) => {
                unsafe {
                    self.mark_as_clean();
                }
                Some(Ok(()))
            }
            Err(e) => Some(Err(e)),
        }
    }

    unsafe fn mark_as_clean(&self) {
        //mark it as cleanly flushing
        let f = (&self.dirty as *const bool)
            .cast_mut()
            .as_mut()
            .unwrap_unchecked();
        *f = false;
    }

    pub fn deref(&self) -> &T {
        if let Some(f) = self.inner.as_ref() {
            return f;
        }

        let mut file_clone = self.file.try_clone().unwrap();

        let val: T = T::deserialize_minimal(&mut file_clone, &self.deserialize_data).unwrap();

        //assign to nothing to ignore the option
        let _ = self.inner.fill_unsafe(val);

        return self.inner.as_ref().unwrap();
    }

    fn ensure_filled(&mut self) {
        if self.inner.is_empty() {
            self.file.rewind().unwrap();

            self.inner.fill(
                T::deserialize_minimal(&mut self.file, &self.deserialize_data).unwrap()
            );
        }
    }

    pub fn ref_mut<'a>(&'a mut self) -> &'a mut T {
        self.ensure_filled();

        self.dirty = true;

        //this reference's lifetime will have to expire in order to allow a future mutable call (such as flush())
        self.inner.as_mut().unwrap()
    }

    pub fn modify<U>(&mut self, func: impl FnOnce(&mut T) -> U) -> U {
        self.ensure_filled();

        self.dirty = true;

        let self_inner = self.inner.as_mut().unwrap();

        let result = func(self_inner);

        result
    }
}

impl<T> Seq<T> {
    pub fn empty() -> Self {
        Seq::InteriorMutable(Cell::new(None))
    }
    pub fn new(value: T) -> Self {
        Seq::Safe(Box::new(value))
    }
    pub fn fill_unsafe(&self, value: T) -> Option<()> {
        match self {
            InteriorMutable(cell) => match unsafe { cell.as_ptr().as_ref() } {
                Some(Some(_already_filled)) => None,
                Some(None) | None => {
                    cell.set(Some(Box::new(value)));
                    Some(())
                }
            },
            Safe(_already_filled) => None,
        }
    }

    pub fn is_filled(&self) -> bool {
        match self {
            InteriorMutable(cell) => unsafe { cell.as_ptr().as_ref().is_some_and(|x| x.is_some()) },
            Safe(_) => true,
        }
    }

    fn is_empty(&self) -> bool {
        return !self.is_filled();
    }

    pub fn is_safe(&self) -> bool {
        match self {
            InteriorMutable(_) => false,
            Safe(_) => true,
        }
    }

    pub fn upgrade_unsafe(&mut self) -> Option<()> {
        match self {
            InteriorMutable(cell) => match cell.take() {
                Some(t) => {
                    *self = Seq::Safe(t);
                    return None;
                }
                None => return None,
            },
            Safe(_) => return Some(()),
        }
    }

    pub fn upgrade_safe(&mut self) -> Option<()> {
        match self {
            InteriorMutable(cell) => match cell.take() {
                Some(t) => {
                    *self = Seq::Safe(t);
                    return None;
                }
                None => return None,
            },
            Safe(_) => return Some(()),
        }
    }

    pub fn fill(&mut self, value: T) -> Option<()> {
        let val = match self {
            InteriorMutable(cell) => match cell.take() {
                Some(t) => {
                    *self = Seq::Safe(t);
                    return None;
                }
                None => value,
            },
            Safe(_) => return None,
        };

        *self = Seq::Safe(Box::new(val));

        return Some(());
    }

    fn as_ref(&self) -> Option<&T> {
        match self {
            InteriorMutable(cell) => match unsafe { cell.as_ptr().as_ref() } {
                Some(Some(t)) => Some(t.as_ref()),
                Some(None) | None => None,
            },
            Safe(ref b) => Some(b.as_ref()),
        }
    }

    fn as_mut(&mut self) -> Option<&mut T> {
        if !self.is_safe() {
            self.upgrade_safe();
        }

        match self {
            InteriorMutable(_) => None,
            Safe(v) => Some(v.as_mut()),
        }
    }
    
    fn free(&mut self)  {
        *self = Seq::InteriorMutable(Cell::new(None));
    }
}
