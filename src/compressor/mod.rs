use std::{io::Write, ops::Deref, path::PathBuf};

use inlining::node::inline_node_tags;
use osmpbfreader::{Node, NodeId, OsmObj, Relation, Tags, Way};
use topn::TopNHeap;
use varint::to_varint;

use crate::tree::{
    bbox::{BoundingBox, EARTH_BBOX},
    LongLatTree,
};

use self::{
    literals::LiteralPool,
    types::{ElementType, KnownRelationTypeTag},
};

mod inlining;
mod literals;
mod tags;
mod topn;
mod types;
mod varint;

pub struct Compressor {
    previous_common_literals: TopNHeap<String, usize>,
    literals: LiteralPool,
    geography: LongLatTree<CompressedOsmData>,
    state_path: PathBuf,
}

struct CompressedOsmData(Vec<u8>);

impl Compressor {
    pub fn new(state_path: PathBuf) -> Self {
        Compressor {
            previous_common_literals: TopNHeap::new(200),
            literals: LiteralPool::new(),
            geography: LongLatTree::new(EARTH_BBOX),
            state_path,
        }
    }
    pub fn write_element(&mut self, element: &mut OsmObj) {
        match element {
            OsmObj::Node(node) => self.write_node(node),
            OsmObj::Way(way) => self.write_way(way),
            OsmObj::Relation(relation) => self.write_relation(relation),
        }
    }

    pub fn write_node(&mut self, node: &mut Node) {
        let way_type = ElementType::Node;

        let inlined = inline_node_tags(&mut node.tags);

        if node.tags.is_empty() {
            return self.write_node_only_inlined_tags(node);
        }
    }

    pub fn write_node_only_inlined_tags(&mut self, node: &mut Node) {
        //first byte layout:

        //1: node
        //0: without any uninlined tags
        //0: has exactly 1 parent? (enables use of next 2 bits for niche-filling)
        //00: start with no parents (represent 0,2,3,more. 0b00 -> 0, 0b01 -> 2; use 0b11 to indicate greater)
        //    if HasExactlyOneParent, then this controls the length (bytes) of the relative pointer to the parent
        //x: has inlined tags
        //xxx: if HasInlinedTags:
        //           index of [None, Tree, PowerTower, PowerPole, Entrance, Bench, Hydrant, Gate]
        //     else:
        //           unused, reserved for future

        //NodeNoTags layout:
        // header (1 byte): as above
        // num_parents (ONLY IF header is MORE parents): varint parent count
        // parent(s): [parent count] iterations of either a varint, or an n-byte-as-per-header int.

        let typ = 0b10_0_0_00_00u8;

        let point_bbox = BoundingBox::from_point(node.decimicro_lat, node.decimicro_lon);

        self.geography
            .insert(point_bbox, CompressedOsmData(vec![typ]))
    }

    fn node_longitude_latitude(node: &Node) -> (Vec<u8>, usize, usize) {
        let mut bytes = Vec::new();

        let lon = to_varint(node.decimicro_lon);
        let lat = to_varint(node.decimicro_lat);

        bytes.extend(lon);
        bytes.extend(lat);

        todo!()
    }

    pub fn write_way(&mut self, way: &Way) {
        let way_type = ElementType::Way;

        todo!();
    }

    pub fn write_relation(&mut self, relation: &Relation) {
        match KnownRelationTypeTag::try_match(&relation.tags) {
            Some(t) => self.write_typed_relation(relation, t),
            None => self.write_untyped_relation(relation),
        }
    }

    fn write_typed_relation(
        &mut self,
        relation: &Relation,
        rel_type: KnownRelationTypeTag,
    ) {
        let broad_discrim = ElementType::RelationTyped.discriminant();

        let has_roles = relation.refs.iter().any(|x| x.role != "");

        let type_byte =
            (broad_discrim << 6) | (rel_type.discriminant() << 1) | (if has_roles { 1 } else { 0 });

            todo!();
    }

    fn write_untyped_relation(
        &mut self,
        relation: &Relation,
    ) {
        todo!();
    }

    pub fn write_tags(&mut self, tags: &Tags, dest: &mut impl Write) {
        for (k, v) in tags.iter() {
            self.write_tag_name(k);
        }
    }

    pub fn write_tag_name(&mut self, name: &impl Deref<Target = str>) {}
}
