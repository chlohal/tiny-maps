use std::{
    collections::VecDeque, fs::{create_dir_all, File}, io::{self, BufWriter}, path::PathBuf
};

use compressed_data::{flattened_id, CompressedOsmData, UncompressedOsmData};
use minimal_storage::{pooled_storage::Pool, serialize_fast::FastMinSerde};
use osm_structures::literal::Literal;
use osm_value_atom::LiteralValue;
use osmpbfreader::{OsmId, OsmObj};

use tree::{
    bbox::{BoundingBox, EARTH_BBOX},
    open_tree,
    point_range::{DisregardWhenDeserializing, StoredBinaryTree},
    StoredTree,
};

pub mod compressed_data;

const CACHE_SATURATION: usize = 4_000;
const DATA_SATURATION: usize = 8_000;

pub struct Compressor {
    values: (Pool<Literal>, Pool<LiteralValue>),
    pub cache_bboxes: StoredBinaryTree<CACHE_SATURATION, u64, BoundingBox<i32>>,
    pub geography: StoredTree<2, DATA_SATURATION, BoundingBox<i32>, UncompressedOsmData>,
    queue_to_handle_at_end: VecDeque<OsmObj>,
}

impl Compressor {
    pub fn new(state_path: &PathBuf) -> Self {
        create_dir_all(state_path).unwrap();

        let lit_file = open_file_with_write(&state_path.join("literals"));
        let val_file = open_file_with_write(&state_path.join("values"));

        let mut geography = open_tree::<2, DATA_SATURATION, BoundingBox<i32>, UncompressedOsmData>(
            state_path.join("geography"),
            EARTH_BBOX,
        );

        let mut cache_bboxes = open_tree::<
            1,
            CACHE_SATURATION,
            u64,
            DisregardWhenDeserializing<u64, FastMinSerde<BoundingBox<i32>>>,
        >(state_path.join("tmp.bboxes"), 0..=u64::MAX);

        geography.expand_to_depth(5);
        cache_bboxes.expand_to_depth(5);

        Compressor {
            values: (
                Pool::new(Box::new(lit_file)).unwrap(),
                Pool::new(Box::new(val_file)).unwrap(),
            ),
            cache_bboxes,
            geography,
            queue_to_handle_at_end: VecDeque::new(),
        }
    }
    pub fn get_element_bbox(&self, id: &OsmId) -> Option<BoundingBox<i32>> {
        let f = self
            .cache_bboxes
            .find_first_item_at_key_exact(&flattened_id(id))
            .map(|x| x.inner().to_owned());
        f.map(|x| x.0)
    }
    pub fn write_element(&mut self, element: OsmObj) {
        let data = CompressedOsmData::make_from_obj(element, &mut self.cache_bboxes);

        let data = match data {
            Ok(data) => data,
            Err(element) => {
                self.queue_to_handle_at_end.push_back(element);
                return;
            }
        };

        let bbox = data.bbox();

        let data = UncompressedOsmData::new(&data, &mut self.values);

        self.geography.insert(bbox, data)
    }

    pub fn flush_to_storage(&mut self) -> Result<(), io::Error> {
        self.geography.flush().unwrap();
        self.cache_bboxes.flush().unwrap();

        self.values.0.flush()?;
        self.values.1.flush()?;

        Ok(())
    }

    pub fn attempt_retry_queue<'a>(&'a mut self) -> impl Iterator<Item = OsmObj> + 'a {
        let mut len = self.queue_to_handle_at_end.len();

        //try 5 times to reduce the size
        for attempt in 0..5 {
            println!("Attempt {attempt}/4 to reduce retry queue:");
            //keep going as long as the size reduces. if it stays the same,
            //then fall through to another of the 5 previous tries.
            loop {
                println!("{len} items in retry queue...");
                for _ in 0..len {
                    let elem = self.queue_to_handle_at_end.pop_front().unwrap();

                    self.write_element(elem);
                }

                if self.queue_to_handle_at_end.len() == len {
                    break;
                }
            }
            len = self.queue_to_handle_at_end.len();
        }

        self.queue_to_handle_at_end.drain(..)
    }
}

fn open_file_with_write(path: &PathBuf) -> File {
    File::options()
        .create(true)
        .read(true)
        .write(true)
        .open(&path)
        .unwrap()
}
