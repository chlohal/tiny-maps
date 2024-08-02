use std::{env, fs::File};

use minimal_storage::{serialize_min::DeserializeFromMinimal, varint::from_varint};
use offline_tiny_maps::compressor::compressed_data::UncompressedOsmData;

use tree::bbox::{BoundingBox, DeltaFriendlyU32Offset};

fn main() {
    let mut file = File::open(env::current_dir().unwrap().join(".map/geography/30")).unwrap();
    
    let num: usize = from_varint(&mut file).unwrap();

    dbg!(num);

    for _ in 0..num {
        let dfuo = DeltaFriendlyU32Offset::deserialize_minimal(&mut file, ()).unwrap();

        let data = UncompressedOsmData::deserialize_minimal(&mut file, &BoundingBox::empty()).unwrap();
        
        println!("reading node 0x30 {:?}", (&dfuo, &data));
    }
}