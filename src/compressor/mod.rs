use std::{
    collections::VecDeque,
    fs::{create_dir_all, File},
    io::{self, BufWriter},
    path::PathBuf,
    rc::Rc,
};

use compressed_data::{flattened_id, unflattened_id, CompressedOsmData, UncompressedOsmData};
use literals::{literal_value::LiteralValue, Literal};
use osmpbfreader::{OsmId, OsmObj, Relation, Way};

use crate::tree::{
    bbox::{BoundingBox, EARTH_BBOX},
    open_tree,
    point_range::{DisregardWhenDeserializing, Point, PointRange},
    LongLatTree, StoredPointTree, StoredTree,
};

use self::{
    literals::LiteralPool,
    types::{ElementType, KnownRelationTypeTag},
};

mod compressed_data;
mod inlining;
mod is_final;
mod literals;
mod topn;
mod types;
pub mod varint;

pub struct Compressor {
    values: (LiteralPool<Literal>, LiteralPool<LiteralValue>),
    pub cache_bboxes: StoredPointTree<1, Point<u64>, BoundingBox<i32>>,
    pub geography: StoredTree<2, BoundingBox<i32>, UncompressedOsmData>,
    queue_to_handle_at_end: VecDeque<OsmObj>,
}

impl Compressor {
    pub fn new(state_path: PathBuf) -> Self {
        create_dir_all(&state_path).unwrap();

        let lit_file = BufWriter::new(open_file_with_write(&state_path.join("literals")));
        let val_file = BufWriter::new(open_file_with_write(&state_path.join("values")));

        let mut geography = open_tree::<2, BoundingBox<i32>, UncompressedOsmData>(
            state_path.join("geography"),
            EARTH_BBOX,
        );

        let mut cache_bboxes = open_tree::<
            1,
            Point<u64>,
            DisregardWhenDeserializing<Point<u64>, BoundingBox<i32>>,
        >(state_path.join("tmp.bboxes"), PointRange(0, u64::MAX));

        geography.ref_mut().expand_to_depth(5);
        cache_bboxes.ref_mut().expand_to_depth(5);

        Compressor {
            values: (
                LiteralPool::new(Box::new(lit_file)),
                LiteralPool::new(Box::new(val_file)),
            ),
            cache_bboxes,
            geography,
            queue_to_handle_at_end: VecDeque::new(),
        }
    }
    pub fn get_element_bbox(&self, id: &OsmId) -> Option<&BoundingBox<i32>> {
        let f = self.cache_bboxes.deref().find_first_item_at_key_exact(&Point(flattened_id(id))).map(|x| x.inner());
        f
    }
    pub fn get_elements_bbox_in_range<'a>(&'a self, range: &'a PointRange<u64>) -> impl Iterator<Item = (OsmId, &'a BoundingBox<i32>)> + 'a {
        self.cache_bboxes.deref().find_entries_in_box(range).map(|(Point(id), bbox)| {
            (unflattened_id(id), bbox.inner())
        })
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

        self.geography.ref_mut().insert(bbox, data)
    }

    pub fn flush_to_storage(&mut self) -> Result<(), io::Error> {
        self.geography.flush(()).unwrap()?;
        self.cache_bboxes.flush(()).unwrap()?;

        self.values.0.flush()?;
        self.values.1.flush()?;

        Ok(())
    }
    
    pub fn handle_retry_queue(&mut self) {
        while let Some(elem) = self.queue_to_handle_at_end.pop_front() {
            eprintln!("Attempting to handle retry queue -- {} items ({:?})", self.queue_to_handle_at_end.len(), elem.id());
            self.write_element(elem);
        }
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
