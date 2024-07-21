use std::{
    fs::create_dir_all,
    io::{self, Write},
    ops::Deref,
    path::PathBuf,
};

use inlining::{
    node::{inline_node_tags, NodeSingleInlined},
    InlinedTags,
};
use literals::{literal_value::LiteralValue, structured_elements::public_transit, Literal};
use osmpbfreader::{Node, OsmObj, Relation, Tags, Way};
use serde::{Deserialize, Serialize};
use varint::to_varint;

use crate::{
    storage::Storage,
    tree::{
        bbox::{BoundingBox, EARTH_BBOX},
        LongLatTree,
    },
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
    values: Storage<(LiteralPool<Literal>, LiteralPool<LiteralValue>)>,
    geography: Storage<LongLatTree<CompressedOsmData>>,
    state_path: PathBuf,
}

#[derive(Serialize, Deserialize, Default)]
struct CompressedOsmData(Vec<u8>);

impl Compressor {
    pub fn new(state_path: PathBuf) -> Self {
        let geo_dir = state_path.join("geography");
        create_dir_all(&geo_dir);

        Compressor {
            values: Storage::new(state_path.join("values"), (LiteralPool::new(), LiteralPool::new())),
            geography: Storage::new(geo_dir.join("root"), LongLatTree::new(EARTH_BBOX, geo_dir)),
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
        let Node {
            id,
            decimicro_lat,
            decimicro_lon,
            ref mut tags,
        } = node;

        let inlined = inline_node_tags(tags);

        let point_bbox = BoundingBox::from_point(node.decimicro_lat, node.decimicro_lon);

        if inlined.other.is_empty() && !inlined.inline.is_multiple() {
            match inlined.inline {
                inlining::node::Node::None => {
                    return self.write_node_only_single_inlined_tags(point_bbox, None)
                }
                inlining::node::Node::Single(tag) => {
                    self.write_node_only_single_inlined_tags(point_bbox, Some(tag))
                }
                inlining::node::Node::Multiple(_, _) => {
                    self.write_node_with_uninlined_tags(point_bbox, inlined)
                }
            }
        }
    }

    fn write_node_with_uninlined_tags(
        &mut self,
        point_bbox: BoundingBox<i32>,
        tags: InlinedTags<inlining::node::Node>,
    ) {
        //header layout:
        //1: node
        //1: with some uninlined tags
        //0: has exactly 1 parent? (enables use of next 2 bits for niche-filling)
        //00: start with no parents (represent 0,2,3,more. 0b00 -> 0, 0b01 -> 2; use 0b11 to indicate greater)
        //    if HasExactlyOneParent, then this controls the length (bytes + 1) of the relative pointer to the parent
        //0000: number of non-inlined tags. 0b1111 => More
        //     see also NodeNoTags

        //specialized tags byte layout:
        // x: node contains an address
        // x: node contains public transport information in the Public Transit schema
        // x: node contains shop and/or amenity information
        // x: node contains a name

        // x: UseSingleInlinedTags
        //   if UseSingleInlinedTags?
        //        xxx: index of [None, Tree, PowerTower, PowerPole, BroadleavedTree, Bench, Hydrant, NeedleleavedTree]
        //   else:
        //      x: node contains a highway feature
        //      x: node contains a place definition
        //      x: node contains operator information

        //NodeBitflagTags layout:
        // header (1 byte): as above
        // specialized_tags (1 byte): as above
        // num_parents (ONLY IF header is MORE parents): varint parent count
        // parent(s): [parent count] iterations of either a varint, or an n-byte-as-per-header int.
        // num_tags (ONLY IF header is MORE tags): varint uninlined tag count
        // tags: [tag count] iterations of compressed tag references.

        let mut typ = 0b10_00_0_000u8;

        let mut tags_byte = 0u8;

        let mut blob = Vec::<u8>::new();

        let InlinedTags {
            inline: inline_tags,
            other: mut non_inlined_tags,
        } = tags;

        //decide whether we're UseingSingleInlineTags.
        //if we are, un-inline highway features, place definitions, and operator information (they overlap)
        let multiple_inlined = match inline_tags {
            inlining::node::Node::Single(_) | inlining::node::Node::None => unreachable!(),
            inlining::node::Node::Multiple(None, other) => other,
            inlining::node::Node::Multiple(Some(single), mut other_inlined) => {
                tags_byte |= 0b1_000;
                tags_byte |= single as u8;

                if let Some(highway) = other_inlined.highway.take() {
                    non_inlined_tags.insert("highway".into(), highway.into());
                }

                if let Some(place) = other_inlined.place.take() {
                    non_inlined_tags.insert("place".into(), place.into());
                }

                if let Some(operator) = other_inlined.operator.take() {
                    non_inlined_tags.insert("operator".into(), operator.into());
                }

                other_inlined
            }
        };

        blob.push(typ);
        blob.push(0u8);

        if non_inlined_tags.len() < 0b1111 {
            typ |= non_inlined_tags.len() as u8;
        } else {
            typ |= 0b1111;

            blob.extend(to_varint(non_inlined_tags.len()));
        }

        let values = self.values.deref_mut();

        if let Some(address) = multiple_inlined.address {
            LiteralPool::<Literal>::insert(values, &address);
            tags_byte |= 0b1000_0000;
        }

        if let Some(public_transit) = multiple_inlined.public_transit {
            LiteralPool::<Literal>::insert(values, &public_transit);
            tags_byte |= 0b0100_0000;
        }

        if let Some(shop) = multiple_inlined.shop {
            LiteralPool::<Literal>::insert(values, &shop);
            tags_byte |= 0b0010_0000;
        }

        if let Some(name) = multiple_inlined.name {
            LiteralPool::<LiteralValue>::insert(&mut values.1, &name);
            tags_byte |= 0b0001_0000;
        }

        if let Some(highway) = multiple_inlined.highway {
            LiteralPool::<LiteralValue>::insert(&mut values.1, &highway);
            tags_byte |= 0b0000_0100;
        }
        if let Some(place) = multiple_inlined.place {
            LiteralPool::<LiteralValue>::insert(&mut values.1, &place);
            tags_byte |= 0b0000_0010;
        }
        if let Some(operator) = multiple_inlined.operator {
            LiteralPool::<LiteralValue>::insert(&mut values.1, &operator);
            tags_byte |= 0b0000_0001;
        }

        //we put in a placeholder earlier; time to place the tags byte into the buffer!
        blob[1] = tags_byte;

        self.geography
            .deref_mut()
            .insert(point_bbox, CompressedOsmData(blob))
    }

    pub fn write_node_only_single_inlined_tags(
        &mut self,
        point_bbox: BoundingBox<i32>,
        tag: Option<NodeSingleInlined>,
    ) {
        //first byte layout:

        //1: node
        //0: without any uninlined tags
        //0: has exactly 1 parent? (enables use of next 2 bits for niche-filling)
        //00: start with no parents (represent 0,2,3,more. 0b00 -> 0, 0b01 -> 2; use 0b11 to indicate greater)
        //    if HasExactlyOneParent, then this controls the length (bytes) of the relative pointer to the parent
        //0: 0 if HasSingleInlinedTags. The 1 option would free up the next 3 bits, but isn't used for anything currently.
        //xxx: if HasSingleInlinedTags:
        //           index of [None, Tree, PowerTower, PowerPole, BroadleavedTree, Bench, Hydrant, NeedleleavedTree]
        //

        //NodeNoTags layout:
        // header (1 byte): as above
        // num_parents (ONLY IF header is MORE parents): varint parent count
        // parent(s): [parent count] iterations of either a varint, or an n-byte-as-per-header int.

        let mut typ = 0b10_00_0_000u8;

        if let Some(tag) = tag {
            typ |= tag as u8;
        }

        let typ = typ;

        self.geography
            .deref_mut()
            .insert(point_bbox, CompressedOsmData(vec![typ]))
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

        todo!();
    }

    fn write_untyped_relation(&mut self, relation: &Relation) {
        todo!();
    }

    pub fn write_tags(&mut self, tags: &Tags, dest: &mut impl Write) {
        for (k, v) in tags.iter() {
            self.write_tag_name(k);
        }
    }

    pub fn write_tag_name(&mut self, name: &impl Deref<Target = str>) {}

    pub fn flush_to_storage(&self) -> Result<(), io::Error> {
        self.geography.flush().unwrap()?;

        self.values.flush().unwrap()?;

        Ok(())
    }
}
