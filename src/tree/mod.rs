use std::{
    collections::{btree_set, BTreeSet},
    mem,
    path::PathBuf,
    rc::Rc,
};

use bbox::{BoundingBox, LongLatSplitDirection};
use compare_by::BoundingBoxOrderedByXOrY;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use crate::storage::Storage;

pub mod bbox;
pub mod compare_by;
mod tree;

mod tree_serde;

const NODE_SATURATION_POINT: usize = 2000;

pub use tree::*;