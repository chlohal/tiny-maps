use std::{env, fs::File, io::Write};

use clap::Parser;
use minimal_storage::packed_string_serialization::is_final::IterIsFinal;
use offline_tiny_maps::compressor::Compressor;

use osmpbfreader::blobs::result_blob_into_iter;

use par_map::ParMap;

const WRITE_EVERY_N_CHUNKS: usize = 1;

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
    let blob_count =
        osmpbfreader::OsmPbfReader::new(File::open(&args.osmpbf).expect("File doesn't exist!"))
            .blobs()
            .count();
        

    let mut blobs = reader
        .blobs();
    let mut blobs_done = 0;

    loop {
        let completed = std::thread::scope(|scope| {
            let mut finished = 0;
            for _ in 0..WRITE_EVERY_N_CHUNKS {
                if let Some(blob) = blobs.next() {
                    dbg!("chunk :)");
                    finished += scope.spawn(|| {
                        let objs = result_blob_into_iter(blob);

                        for obj in objs {
                            if let Ok(obj) = obj {
                                compressor.write_element(obj)
                            }
                        }

                        1
                    }).join().unwrap();
                }
            }

            finished
        });
        dbg!(completed);
        blobs_done += completed;

        println!("{blobs_done}/{blob_count} chunks finished");
        compressor.flush_to_storage().unwrap();

        if completed == 0 {
            break;
        }
    }

    println!("moving on to the incomplete relations");

    let incompleted_relations = compressor.attempt_retry_queue();

    let mut incomplete_file =
        std::fs::File::create(&state_dir.join("incomplete_relations.note")).unwrap();

    writeln!(&mut incomplete_file, "Incomplete relations:").unwrap();
    for item in incompleted_relations {
        writeln!(&mut incomplete_file, "{:?}", item.id()).unwrap();
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
