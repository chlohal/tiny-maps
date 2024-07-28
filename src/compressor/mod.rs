use std::{
    fs::create_dir_all,
    io::{self, BufWriter},
    path::PathBuf,
    rc::Rc,
};

use compressed_data::{CompressedOsmData, UncompressedOsmData};
use literals::{literal_value::LiteralValue, Literal};
use osmpbfreader::{OsmObj, Relation, Way};

use crate::
    tree::{
        bbox::{BoundingBox, EARTH_BBOX}, open_tree, point_range::{DisregardWhenDeserializing, Point, PointRange}, LongLatTree, StoredPointTree, StoredTree
    }
;

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
    cache_bboxes: StoredPointTree<1, Point<u64>, BoundingBox<i32>>,
    geography: StoredTree<2, BoundingBox<i32>, UncompressedOsmData>,
    state_path: PathBuf,
}

impl Compressor {
    pub fn new(state_path: PathBuf) -> Self {
        create_dir_all(&state_path).unwrap();

        let lit_file = BufWriter::new(std::fs::File::create(state_path.join("literals")).unwrap());
        let val_file = BufWriter::new(std::fs::File::create(state_path.join("values")).unwrap());

        let mut geography = open_tree::<2, BoundingBox<i32>, UncompressedOsmData>(
            state_path.join("geography"),
            EARTH_BBOX,
        );

        let cache_bboxes = open_tree::<
            1,
            Point<u64>,
            DisregardWhenDeserializing<Point<u64>, BoundingBox<i32>>,
        >(state_path.join("tmp.bboxes"), PointRange(0, u64::MAX));

        geography.ref_mut().expand_to_depth(5);

        Compressor {
            values: (
                LiteralPool::new(Box::new(lit_file)),
                LiteralPool::new(Box::new(val_file)),
            ),
            cache_bboxes,
            geography,
            state_path,
        }
    }
    pub fn write_element(&mut self, element: OsmObj) {
        let data = CompressedOsmData::make_from_obj(element, &mut self.cache_bboxes);
        let bbox = data.bbox();

        let data = UncompressedOsmData::new(&data, &mut self.values);

        self.geography.ref_mut().insert(bbox, data)
    }

    pub fn write_way(&mut self, way: &Way) {
        //way header layout:
        //0: not node
        //1: way
        //xxxx: child count (0b1111 for MORE)
        //xx:

        let typ = 0b01u8;
    }

    pub fn write_relation(&mut self, relation: &Relation) {
        match KnownRelationTypeTag::try_match(&relation.tags) {
            Some(t) => self.write_typed_relation(relation, t),
            None => self.write_untyped_relation(relation),
        }
    }

    fn write_typed_relation(&mut self, relation: &Relation, rel_type: KnownRelationTypeTag) {
        let broad_discrim = ElementType::RelationTyped.discriminant();

        let has_roles = relation.refs.iter().any(|x| x.role != "");

        let type_byte =
            (broad_discrim << 6) | (rel_type.discriminant() << 1) | (if has_roles { 1 } else { 0 });
    }

    fn write_untyped_relation(&mut self, relation: &Relation) {}

    pub fn flush_to_storage(&mut self) -> Result<(), io::Error> {
        self.geography.flush(()).unwrap()?;
        self.cache_bboxes.flush(()).unwrap()?;

        self.values.0.flush()?;
        self.values.1.flush()?;

        Ok(())
    }
}
fn make_geography(state_path: &PathBuf) -> StoredTree<2, BoundingBox<i32>, UncompressedOsmData> {
    let geo_dir = state_path.join("geography");
    create_dir_all(&geo_dir).unwrap();

    let tree_structure_file = std::fs::File::options()
        .create(true)
        .write(true)
        .read(true)
        .open(geo_dir.join("structure"))
        .unwrap();

    let geo_dir_rc = Rc::new((geo_dir.clone(), tree_structure_file));

    let geography = StoredTree::<2, BoundingBox<i32>, UncompressedOsmData>::new(
        geo_dir.join("root"),
        LongLatTree::<2, BoundingBox<i32>, UncompressedOsmData>::new(
            EARTH_BBOX,
            Rc::clone(&geo_dir_rc),
        ),
        (geo_dir_rc, 1, EARTH_BBOX),
    );
    geography
}
