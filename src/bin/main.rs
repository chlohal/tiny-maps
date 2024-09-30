use std::{env, fs::File, io::Write};

use clap::Parser;
use offline_tiny_maps::compressor::Compressor;

use osmpbfreader::blobs::result_blob_into_iter;

use par_map::ParMap;

const WRITE_EVERY_N_CHUNKS: usize = 8;

fn main() {
    let args = Args::parse();

    let file = File::open(&args.osmpbf).expect("File doesn't exist!");

    let mut reader = osmpbfreader::OsmPbfReader::new(&file);

    let state_dir = env::current_dir()
    .unwrap()
    .join(args.output.unwrap_or(".map".into()));

    let mut compressor = Compressor::new(&state_dir);

    //we need to make a new reader in order to get the blob count, but this iterator is much faster than anything else b/c it doesn't need to 
    //decompress or process
    let blob_count = osmpbfreader::OsmPbfReader::new(File::open(&args.osmpbf).expect("File doesn't exist!")).blobs().count();

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
            println!("Chunk {blob_id}/{blob_count} finished");
            compressor.flush_to_storage().unwrap();
            blobs_since_last_write = 0;
        }
    }

    println!("moving on to the incomplete relations");

    let incompleted_relations = compressor.attempt_retry_queue();

    let mut incomplete_file = std::fs::File::create(&state_dir.join("incomplete_relations.note")).unwrap();

    writeln!(&mut incomplete_file, "Incomplete relations:").unwrap();
    for item in incompleted_relations {
        writeln!(&mut incomplete_file, "{}", item.id().inner_id()).unwrap();
    }

    compressor.flush_to_storage().unwrap();
}

#[derive(Parser, Debug)]
struct Args {
    /// osm.pbf file to load
    osmpbf: String,

    /// directory to output data to. Default: `.map`
    output: Option<String>,
}
