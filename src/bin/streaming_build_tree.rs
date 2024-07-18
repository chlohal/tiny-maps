use std::{env::{self, args_os}, fs::File, os};

use offline_tiny_maps::{compressor::Compressor, postgres_objects::{NODE_OBJ, RELATION_OBJ, WAY_OBJ}};
use osmpbfreader::{blobs::result_blob_into_iter, OsmId};

use par_map::ParMap;

fn main() -> Result<(), postgres::Error> {
    let filename = args_os()
        .nth(1)
        .expect("Usage: tiny-map-postgres-import [OSMPBF file]");
    let file = File::open(filename).expect("File doesn't exist!");

    let mut reader = osmpbfreader::OsmPbfReader::new(file);

    let mut compressor = Compressor::new(env::current_dir().unwrap().join("Output"));

    let blobs = reader
        .blobs()
        .enumerate()
        .par_flat_map(|(i, x)| result_blob_into_iter(x).map(move |x| (i,x)));

    for (id, obj) in blobs {
        let Ok(mut obj) = obj else { continue; };

        let type_id = osm_type_id(&obj.id());

        let obj_id = obj.id().inner_id();

        compressor.write_element(&mut obj)
    }

    Ok(())
}

fn osm_type_id(member: &OsmId) -> i16 {
    if member.is_node() {
        return NODE_OBJ;
    }
    if member.is_way() {
        return WAY_OBJ;
    }
    return RELATION_OBJ;
}
