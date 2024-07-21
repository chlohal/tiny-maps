use std::{
    cell::Cell,
    fs::File,
    io::{self, Seek},
    ops::{Deref, DerefMut},
    os::fd::AsFd,
    path::PathBuf,
};

use serde::{de::DeserializeOwned, ser::Error, Deserialize, Serialize};

enum Seq<T> {
    InteriorMutable(Cell<Option<Box<T>>>),
    Safe(Box<T>),
}

use Seq::*;

pub struct Storage<T> {
    inner: Seq<T>,
    dirty: bool,
    path: PathBuf,
    file: File,
}

impl<T: Serialize + DeserializeOwned> Serialize for Storage<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        //only attempt to flush if it's dirty
        if false && self.dirty {
            match self.flush() {
                Some(t) => t.map_err(|_| S::Error::custom("Unable to flush Storage"))?,
                None => {}
            }
        }

        PathBuf::serialize(&self.path, serializer)
    }
}

impl<'de, T: Serialize + DeserializeOwned> Deserialize<'de> for Storage<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        PathBuf::deserialize(deserializer).map(|x| Storage::open(x))
    }
}

impl<'de, T: Serialize + DeserializeOwned> Storage<T> {
    pub fn new(id: PathBuf, value: T) -> Self {
        let file = File::options()
            .create(true)
            .read(true)
            .write(true)
            .open(&id)
            .unwrap();

        bincode::serialize_into(&file, &value).unwrap();

        Self {
            inner: Seq::new(value),
            path: id,
            file,
            dirty: false,
        }
    }

    pub fn open(id: PathBuf) -> Self {
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
        }
    }

    pub fn flush(&self) -> Option<Result<(), io::Error>> {
        let value = self.inner.as_ref()?;

        //this whole struct is non-sync, so we can do this fearlessly
        unsafe {
            let fd = &self.file as *const File;

            fd.cast_mut().as_mut().unwrap_unchecked().rewind().unwrap();
        }
        self.file.set_len(0).unwrap();

        let e = bincode::serialize_into(&self.file, value)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e));

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

    pub fn modify<U>(&mut self, mut callback: impl FnMut(&mut T) -> U) -> U {
        self.file.rewind().unwrap();

        let file_clone = self.file.try_clone().unwrap();

        self.dirty = true;

        let val = self.deref_mut();

        let result = callback(val);

        bincode::serialize_into(file_clone, val).unwrap();

        result
    }

    pub fn deref(&self) -> &T {
        if let Some(f) = self.inner.as_ref() {
            return f;
        }

        let val: T = bincode::deserialize_from(&self.file).unwrap();

        //assign to nothing to ignore the option
        let _ = self.inner.fill_unsafe(val);

        return self.inner.as_ref().unwrap();
    }

    pub fn deref_mut(&mut self) -> &mut T {
        if self.inner.is_empty() {
            let val: T = bincode::deserialize_from(&self.file).unwrap();

            //assign to nothing to ignore the option
            let _ = self.inner.fill(val);
        }

        self.dirty = true;

        return self.inner.as_mut().unwrap();
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
