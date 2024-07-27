use std::{
    fs::create_dir_all,
    io::{self, BufWriter, Write},
    ops::Deref,
    path::PathBuf,
    rc::Rc,
};

use compressed_data::{CompressedOsmData, UncompressedOsmData};
use inlining::{
    node::{inline_node_tags, NodeSingleInlined},
    InlinedTags,
};
use literals::{literal_value::LiteralValue, Literal};
use osmpbfreader::{Node, OsmObj, Relation, Tags, Way};

use varint::{from_varint, to_varint};

use crate::{
    storage::{serialize_min::{DeserializeFromMinimal, SerializeMinimal}, Storage},
    tree::{
        bbox::{BoundingBox, EARTH_BBOX},
        LongLatTree, StoredTree,
    },
};

use self::{
    literals::LiteralPool,
    types::{ElementType, KnownRelationTypeTag},
};

mod inlining;
mod is_final;
mod literals;
mod tags;
mod topn;
mod types;
mod compressed_data;
pub mod varint;

pub struct Compressor {
    values: (LiteralPool<Literal>, LiteralPool<LiteralValue>),
    geography: StoredTree<UncompressedOsmData>,
    state_path: PathBuf,
}

impl Compressor {
    pub fn new(state_path: PathBuf) -> Self {

        create_dir_all(&state_path).unwrap();

        let lit_file = BufWriter::new(std::fs::File::create(state_path.join("literals")).unwrap());
        let val_file = BufWriter::new(std::fs::File::create(state_path.join("values")).unwrap());

        let mut geography = make_geography(&state_path);

        geography.modify(|tree| tree.expand_to_depth(5));

        Compressor {
            values: (
                LiteralPool::new(Box::new(lit_file)),
                LiteralPool::new(Box::new(val_file)),
            ),
            geography,
            state_path,
        }
    }
    pub fn write_element(&mut self, element: OsmObj) {
        let data: CompressedOsmData = element.into();
        let bbox = data.bbox().clone();
        let data = UncompressedOsmData::new(data, &mut self.values);

        self.geography.modify(|tree| tree.insert(bbox, data))
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

        self.values.0.flush()?;
        self.values.1.flush()?;

        Ok(())
    }
}

fn make_geography(state_path: &PathBuf) -> StoredTree<UncompressedOsmData> {
    let geo_dir = state_path.join("geography");
    create_dir_all(&geo_dir).unwrap();

    let tree_structure_file = std::fs::File::options().create(true).write(true).read(true).open(geo_dir.join("structure")).unwrap();

    let geo_dir_rc = Rc::new((geo_dir.clone(), tree_structure_file));


    let geography = StoredTree::<UncompressedOsmData>::new(
        geo_dir.join("root"),
        LongLatTree::<UncompressedOsmData>::new(EARTH_BBOX, Rc::clone(&geo_dir_rc)),
        (geo_dir_rc, 1, EARTH_BBOX),
    );
    geography
}
