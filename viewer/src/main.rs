use osm_tag_compression::compressed_data::UncompressedOsmData;
use tree::{
    bbox::{BoundingBox, EARTH_BBOX},
    open_tree_dense,
};

use std::io::{BufWriter, Write};

mod window;
mod cartography;
mod loader;

const DATA_SATURATION: usize = 8_000;

pub fn main() {
    pollster::block_on(window::open::<cartography::State>());
}