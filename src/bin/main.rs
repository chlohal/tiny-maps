use std::{collections::HashSet, env::args_os, fs::File};

use osmpbfreader::OsmObj;


fn main() {
    let filename = args_os().nth(1).expect("Usage: tiny-map-preprocess [OSMPBF file]");
    let file = File::open(filename).expect("File doesn't exist!");

    let mut reader = osmpbfreader::OsmPbfReader::new(file);

    println!("{}", reader.blobs().count());
}
