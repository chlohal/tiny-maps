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

use serialize_min::{DeserializeFromMinimal, SerializeMinimal};
use Seq::*;

pub struct Storage<D, T>
where
    T: SerializeMinimal,
    for<'a> T: DeserializeFromMinimal<ExternalData<'a> = &'a D>,
{
    inner: Seq<T>,
    dirty: bool,
    path: PathBuf,
    file: File,
    deserialize_data: D,
}

pub mod serialize_min;

pub trait StorageReachable: SerializeMinimal {
    fn flush_children<'a>(&'a self, data: Self::ExternalData<'a>) -> Result<(), io::Error> {
        Ok(())
    }
}

impl<D, T> Storage<D, T>
where
    for<'a> <T as SerializeMinimal>::ExternalData<'a>: Copy,
    T: 'static + StorageReachable + SerializeMinimal,
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
            path: id,
            file,
            dirty: false,
            deserialize_data,
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
            path: id,
            file,
            dirty: true,
            deserialize_data,
        }
    }

    pub fn flush<'a>(
        &'a self,
        serialize_data: <T as SerializeMinimal>::ExternalData<'a>,
    ) -> Option<Result<(), io::Error>> {
        if !self.dirty {
            return Some(Ok(()));
        }

        let value = self.inner.as_ref()?;

        let mut file_clone = self.file.try_clone().unwrap();

        //this whole struct is non-sync, so we can do this fearlessly
        file_clone.rewind().unwrap();
        file_clone.set_len(0).unwrap();

        let e = value.minimally_serialize(&mut file_clone, serialize_data);

        match file_clone.flush() {
            Ok(_) => {}
            Err(e) => return Some(Err(e)),
        }

        match value.flush_children(serialize_data) {
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

    pub fn modify<U>(&mut self, func: impl FnOnce(&mut T) -> U) -> U {
        //read and fill if we're empty currently
        if self.inner.is_empty() {
            self.file.rewind().unwrap();

            self.inner.fill(
                T::deserialize_minimal(&mut self.file, &self.deserialize_data).unwrap()
            );
        }

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

    pub fn safe_ref_mut(&mut self) -> Option<&mut T> {
        if !self.is_safe() {
            self.upgrade_safe();
        }

        match self {
            InteriorMutable(_) => None,
            Safe(v) => Some(v.as_mut()),
        }
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
        match self {
            InteriorMutable(cell) => match cell.get_mut() {
                Some(t) => Some(t.as_mut()),
                None => None,
            },
            Safe(ref mut b) => Some(b.as_mut()),
        }
    }
}
