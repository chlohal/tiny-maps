use std::{
    collections::VecDeque, fs::{create_dir_all, File}, io::{self}, path::PathBuf, usize
};

use debug_logs::debug_print;
use parking_lot::Mutex;

use minimal_storage::{pooled_storage::Pool};
use osm_tag_compression::{compressed_data::{CompressedOsmData, UncompressedOsmData}, field::Field};
use osm_value_atom::LiteralValue;
use osmpbfreader::{OsmObj};

use tree::{
    bbox::{BoundingBox, EARTH_BBOX}, open_tree_dense, open_tree_sparse, point_range::StoredBinaryTree, StoredTree
};


const CACHE_SATURATION: usize = 4_000;
const DATA_SATURATION: usize = 8_000;

pub struct Compressor {
    values: (Pool<Field>, Pool<LiteralValue>),
    pub cache_bboxes: StoredBinaryTree<CACHE_SATURATION, u64, BoundingBox<i32>>,
    pub geography: StoredTree<2, DATA_SATURATION, BoundingBox<i32>, UncompressedOsmData>,
    queue_to_handle_at_end: Mutex<VecDeque<OsmObj>>,
}

impl Compressor {
    pub fn new(state_path: &PathBuf) -> Self {
        create_dir_all(state_path).unwrap();

        let lit_file = open_file_with_write(&state_path.join("literals"));
        let val_file = open_file_with_write(&state_path.join("values"));

        let mut geography = open_tree_dense::<2, DATA_SATURATION, BoundingBox<i32>, UncompressedOsmData>(
            state_path.join("geography"),
            EARTH_BBOX,
        );

        let mut cache_bboxes = open_tree_sparse::<
            1,
            CACHE_SATURATION,
            u64,
            BoundingBox<i32>,
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
            queue_to_handle_at_end: Mutex::new(VecDeque::new()),
        }
    }
    pub fn write_element(&self, element: OsmObj) {
        debug_print!("begin");

        let data = CompressedOsmData::make_from_obj(element, &self.cache_bboxes);

        debug_print!("after make_from_obj");

        let data = match data {
            Ok(data) => data,
            Err(element) => {
                self.queue_to_handle_at_end.lock().push_back(element);
                return;
            }
        };

        let bbox = data.bbox();

        debug_assert!(self.geography.root_bbox().contains(&bbox));

        let data = UncompressedOsmData::new(&data, &self.values);

        self.geography.insert(bbox, data)
    }

    pub fn flush_to_storage(&mut self) -> Result<(), io::Error> {
        self.geography.flush()?;
        self.cache_bboxes.flush()?;

        let values = &self.values;
        values.0.flush()?;
        values.1.flush()?;

        Ok(())
    }

    pub fn attempt_retry_queue<'a>(&'a mut self) -> impl Iterator<Item = OsmObj> + 'a {
        //try 5 times to reduce the size
        for attempt in 0..5 {
            println!("Attempt {attempt}/4 to reduce retry queue:");
            //keep going as long as the size reduces. if it stays the same,
            //then fall through to another of the 5 previous tries.
            loop {
                let len = self.queue_to_handle_at_end.lock().len();

                println!("{len} items in retry queue...");
                for _ in 0..len {
                    let elem = self.queue_to_handle_at_end.lock().pop_front().unwrap();

                    self.write_element(elem);
                }

                if self.queue_to_handle_at_end.lock().len() == len {
                    break;
                }
            }
        }

        self.queue_to_handle_at_end.get_mut().drain(..)
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
