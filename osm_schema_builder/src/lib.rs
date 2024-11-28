use std::{
    collections::{HashSet, VecDeque},
    env,
    ffi::OsString,
    fs::{read_dir, DirEntry, File, ReadDir},
    io::Write,
    ops::AddAssign,
    path::{Path, PathBuf},
    result,
};

use serde_json::{self, Value};

pub mod deprecations;
pub mod fields;
pub mod util;
pub mod presets;