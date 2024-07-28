use std::{env, fs::File};

use clap::Parser;
use offline_tiny_maps::compressor::Compressor;
use osmpbfreader::blobs::result_blob_into_iter;

use par_map::ParMap;

const WRITE_EVERY_N_CHUNKS: usize = 1;

fn main() {
    let args = Args::parse();

    let file = File::open(args.osmpbf).expect("File doesn't exist!");

    let mut reader = osmpbfreader::OsmPbfReader::new(file);

    let mut compressor = Compressor::new(
        env::current_dir()
            .unwrap()
            .join(args.output.unwrap_or(".map".into())),
    );

    let blobs = reader
        .blobs()
        .enumerate()
        .par_flat_map(|(i, x)| result_blob_into_iter(x).map(move |x| (i, x)));

    let mut last_blob_id = usize::MAX;
    let mut blobs_since_last_write = 0;

    for (blob_id, obj) in blobs {
        let Ok(obj) = obj else {
            continue;
        };

        compressor.write_element(obj);

        if blob_id != last_blob_id {
            blobs_since_last_write += 1;
            last_blob_id = blob_id;
        }

        if blobs_since_last_write >= WRITE_EVERY_N_CHUNKS {
            compressor.flush_to_storage().unwrap();
            blobs_since_last_write = 0;
        }
    }

    compressor.flush_to_storage().unwrap();

    compressor.handle_retry_queue();

    compressor.flush_to_storage().unwrap();
}

#[derive(Parser, Debug)]
struct Args {
    /// osm.pbf file to load
    osmpbf: String,

    /// directory to output data to. Default: `.map`
    output: Option<String>,
}
