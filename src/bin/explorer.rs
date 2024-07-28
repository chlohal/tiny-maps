use std::env;

use clap::Parser;
use offline_tiny_maps::compressor::Compressor;

use tree::{bbox::EARTH_BBOX, point_range::PointRange};

fn main() {
    let args = Args::parse();

    let mut compressor = Compressor::new(env::current_dir().unwrap().join(args.data_dir));

    for item in compressor.geography.deref().find_entries_in_box(&EARTH_BBOX) {
        dbg!(item);
    }

    for (id, item) in compressor.get_elements_bbox_in_range(&PointRange(0, u64::MAX)) {
        dbg!((id, item));
    }

    dbg!(compressor.get_element_bbox(&osmpbfreader::OsmId::Node(osmpbfreader::NodeId(8569371426))));
}

#[derive(Parser, Debug)]
struct Args {

    /// directory with map data. Default: `.map`
    #[arg(default_value_t = Into::into(".map"))]
    data_dir: String,
}
