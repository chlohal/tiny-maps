pub mod bbox;
pub mod compare_by;
mod tree;

mod tree_serde;

const NODE_SATURATION_POINT: usize = 8_000;

pub use tree::*;