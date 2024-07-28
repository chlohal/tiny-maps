use osmpbfreader::NodeId;

use crate::compressor::tag_compressing::{
            self,
            node::{inline_node_tags, NodeSingleInlined},
            InlinedTags,
        };

use minimal_storage::varint::ToVarint;

use osm_literals::{literal_value::LiteralValue, literal::Literal, pool::LiteralPool};

use tree::bbox::BoundingBox;

use super::CompressedOsmData;

pub fn osm_node_to_compressed_node(node: osmpbfreader::Node) -> CompressedOsmData {
    let id = node.id;
    let tags = inline_node_tags(node.tags);
    let point = BoundingBox::from_point(node.decimicro_lat, node.decimicro_lon);

    CompressedOsmData::Node { id, tags, point }
}

pub fn serialize_node<W: std::io::Write>(
    write_to: &mut W,
    external_data: &mut (LiteralPool<Literal>, LiteralPool<LiteralValue>),
    id: &NodeId,
    tags: &InlinedTags<tag_compressing::node::Node>,
    point: &BoundingBox<i32>,
) -> Result<(), std::io::Error> {
    if tags.other.is_empty() && tags.inline.is_single() {
        return write_node_only_single_inlined_tags(write_to, external_data, point, tags.inline.single.clone(), id);
    }

    write_node_with_uninlined_tags(write_to, external_data, point, tags)
}

pub fn write_node_only_single_inlined_tags<W: std::io::Write>(
    write_to: &mut W,
    values: &mut (LiteralPool<Literal>, LiteralPool<LiteralValue>),
    point_bbox: &BoundingBox<i32>,
    tag: Option<NodeSingleInlined>,
    id: &NodeId,
) -> std::io::Result<()> {
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

    write_to.write_all(&[typ])?;
    id.0.write_varint(write_to)
}

fn write_node_with_uninlined_tags<W: std::io::Write>(
    write_to: &mut W,
    values: &mut (LiteralPool<Literal>, LiteralPool<LiteralValue>),
    point_bbox: &BoundingBox<i32>,
    tags: &InlinedTags<tag_compressing::node::Node>,
) -> std::io::Result<()> {
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

    // x: UNUSED: TODO FILL THIS
    // x: node contains a highway feature
    // x: node contains a place definition
    // x: node contains operator information

    //NodeBitflagTags layout:
    // header (1 byte): as above
    // specialized_tags (1 byte): as above
    // num_parents (ONLY IF header is MORE parents): varint parent count
    // parent(s): [parent count] iterations of either a varint, or an n-byte-as-per-header int.
    // num_tags (ONLY IF header is MORE tags): varint uninlined tag count
    // tags: [tag count] iterations of compressed tag references.

    let typ = 0b10_00_0_000u8;

    let mut tags_byte = 0u8;

    let mut blob = Vec::<u8>::new();

    let InlinedTags {
        inline: ref inline_tags,
        other: ref non_inlined_tags,
    } = tags;

    blob.push(typ);
    blob.push(0u8);

    let multiple_inlined = &inline_tags.multiple;    

    if non_inlined_tags.len() < 0b1111 {
        blob[0] |= non_inlined_tags.len() as u8;
    } else {
        blob[0] |= 0b1111;
        non_inlined_tags.len().write_varint(&mut blob)?;
    }

    if let Some(address) = &multiple_inlined.address {
        let id = LiteralPool::<Literal>::insert(values, &address)?;
        id.write_varint(&mut blob)?;
        tags_byte |= 0b1000_0000;
    }

    if let Some(public_transit) = &multiple_inlined.public_transit {
        let id = LiteralPool::<Literal>::insert(values, &public_transit)?;
        id.write_varint(&mut blob)?;
        tags_byte |= 0b0100_0000;
    }

    if let Some(shop) = &multiple_inlined.shop {
        let id = LiteralPool::<Literal>::insert(values, &shop)?;
        id.write_varint(&mut blob)?;
        tags_byte |= 0b0010_0000;
    }

    if let Some(name) = &multiple_inlined.name {
        let id = LiteralPool::<LiteralValue>::insert(&mut values.1, &name)?;
        id.write_varint(&mut blob)?;
        tags_byte |= 0b0001_0000;
    }

    if let Some(contact) = &multiple_inlined.contact {
        let id = LiteralPool::<Literal>::insert(values, &contact)?;
        id.write_varint(&mut blob)?;
        tags_byte |= 0b0000_0100;
    }
    if let Some(place) = &multiple_inlined.place {
        let id = LiteralPool::<LiteralValue>::insert(&mut values.1, &place)?;
        id.write_varint(&mut blob)?;
        tags_byte |= 0b0000_0010;
    }
    if let Some(operator) = &multiple_inlined.operator {
        let id = LiteralPool::<LiteralValue>::insert(&mut values.1, &operator)?;
        id.write_varint(&mut blob)?;
        tags_byte |= 0b0000_0001;
    }

    //we put in a placeholder earlier; time to place the tags byte into the buffer!
    blob[1] = tags_byte;

    for taglit in non_inlined_tags {
        let id = LiteralPool::<Literal>::insert(values, &taglit)?;
        id.write_varint(&mut blob)?;
    }

    write_to.write_all(&blob)
}
