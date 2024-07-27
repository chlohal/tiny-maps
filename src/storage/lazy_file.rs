use std::{
    fs::File,
    io::{Read, Seek, Write},
    path::PathBuf,
};

pub struct LazyFile {
    path: PathBuf,
    file: Option<File>,
}

impl Write for LazyFile {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if buf.len() > 0 {
            self.assure_created().write(buf)
        } else {
            Ok(0)
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match &mut self.file {
            Some(f) => f.flush(),
            None => Ok(()),
        }
    }
}

impl Read for LazyFile {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.assure_created().read(buf)
    }
}

impl Seek for LazyFile {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        let need_to_create_file = match pos {
            std::io::SeekFrom::Current(0) | std::io::SeekFrom::Start(0) => true,
            _ => false,
        };

        if need_to_create_file {
            self.assure_created().seek(pos)
        } else {
            return Ok(0);
        }
    }

    fn rewind(&mut self) -> std::io::Result<()> {
        match &mut self.file {
            Some(f) => f.rewind(),
            None => Ok(()),
        }
    }

    fn stream_position(&mut self) -> std::io::Result<u64> {
        match &mut self.file {
            Some(f) => f.stream_position(),
            None => Ok(0),
        }
    }
}

impl LazyFile {
    pub fn set_len(&self, size: u64) -> std::io::Result<()> {
        match &self.file {
            Some(f) => f.set_len(size),
            None => {
                if size == 0 {
                    Ok(())
                } else {
                    self.open_create(|f| f.set_len(size))
                }
            }
        }
    }

    pub fn try_clone(&self) -> std::io::Result<LazyFile> {
        match &self.file {
            Some(f) => Ok(Self {
                path: self.path.clone(),
                file: Some(f.try_clone()?),
            }),
            None => Ok(Self {
                path: self.path.clone(),
                file: Some(self.create()),
            }),
        }
    }

    fn open_create<U>(&self, run_with: impl FnOnce(&File) -> U) -> U {
        if let Some(f) = &self.file {
            return run_with(f);
        }

        run_with(&self.create())
    }

    fn create(&self) -> File {
        File::options()
            .create(true)
            .read(true)
            .write(true)
            .open(&self.path)
            .unwrap()
    }

    fn assure_created(&mut self) -> &mut File {
        if self.file.is_none() {
            self.file = Some(self.create());
        }

        self.file.as_mut().unwrap()
    }

    pub fn new(path: PathBuf) -> Self {
        Self { path, file: None }
    }
}
