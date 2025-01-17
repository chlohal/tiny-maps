use osm_structures::structured_elements::{address::OsmAddress, contact::OsmContactInfo};
use osm_value_atom::LiteralValue;
use osmpbfreader::{Node, NodeId, Tags};

use crate::{field::Field, removable::remove_non_stored_tags};

use minimal_storage::{pooled_storage::Pool, serialize_min::SerializeMinimal, varint::ToVarint};

use tree::bbox::BoundingBox;

use super::{CompressedOsmData, Fields};

pub fn osm_node_to_compressed_node(node: osmpbfreader::Node) -> CompressedOsmData {
    let id = node.id;
    let tags = inline_node_tags(node.tags);
    let point = BoundingBox::from_point(node.decimicro_lat, node.decimicro_lon);

    CompressedOsmData::Node { id, tags, point }
}

pub fn serialize_node<W: std::io::Write>(
    write_to: &mut W,
    external_data: &(Pool<Field>, Pool<LiteralValue>),
    id: &NodeId,
    tags: &NodeFields,
) -> Result<(), std::io::Error> {
    match tags {
        NodeFields::Single(s) => write_node_only_single_inlined_tags(write_to, s, id),
        NodeFields::Multiple(f) => write_node_with_uninlined_tags(write_to, external_data, f),
    }    
}

pub fn write_node_only_single_inlined_tags<W: std::io::Write>(
    write_to: &mut W,
    tag: &Option<NodeSingleInlined>,
    id: &NodeId,
) -> std::io::Result<()> {
    //first byte layout:

    //1: node
    //0: without any uninlined tags
    //0: has exactly 1 parent.unwrap() (enables use of next 2 bits for niche-filling)
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
        typ |= *tag as u8;
    }

    write_to.write_all(&[typ]).unwrap();
    id.0.write_varint(write_to)
}

fn write_node_with_uninlined_tags<W: std::io::Write>(
    write_to: &mut W,
    (literals, values): &(Pool<Field>, Pool<LiteralValue>),
    tags: &Fields,
) -> std::io::Result<()> {
    //header layout:
    //1: node
    //1: with some uninlined tags
    //0: has exactly 1 parent.unwrap() (enables use of next 2 bits for niche-filling)
    //00: start with no parents (represent 0,2,3,more. 0b00 -> 0, 0b01 -> 2; use 0b11 to indicate greater)
    //    if HasExactlyOneParent, then this controls the length (bytes + 1) of the relative pointer to the parent
    //0000: number of non-inlined tags. 0b1111 => More
    //     see also NodeNoTags

    //NodeBitflagTags layout:
    // header (1 byte): as above
    // num_parents (ONLY IF header is MORE parents): varint parent count
    // parent(s): [parent count] iterations of either a varint, or an n-byte-as-per-header int.
    // num_tags (ONLY IF header is MORE tags): varint uninlined tag count
    // tags: [tag count] iterations of compressed tag references.

    let Fields(fields) = tags;

    let mut typ = 0b10_00_0_000u8;

    if fields.len() < 0b1111 {
        typ |= fields.len() as u8;
        typ.minimally_serialize(write_to, ())?;
    } else {
        typ |= 0b1111;
        typ.minimally_serialize(write_to, ())?;
        fields.len().write_varint(write_to)?;
    }

    for taglit in fields.iter() {
        let id = Pool::<Field>::insert(literals, &taglit, values).unwrap();
        id.write_varint(write_to)?;
    }

    Ok(())
}


#[derive(Clone, Debug)]
pub enum NodeFields {
    Single(Option<NodeSingleInlined>),
    Multiple(Fields)
}

#[derive(Clone, Copy, Debug)]
#[repr(u8)]
pub enum NodeSingleInlined {
    Tree = 1,
    PowerTower = 2,
    PowerPole = 3,
    BroadleavedTree = 4,
    Bench = 5,
    Hydrant = 6,
    NeedleleavedTree = 7,
}

pub fn inline_node_tags(mut tags: osmpbfreader::Tags) -> NodeFields {
    remove_non_stored_tags(&mut tags);

    if tags.len() == 0 {
        return NodeFields::Single(None);
    }

    if tags.len() == 1 {
        let only_tag = tags.iter().next().unwrap();

        let mut try_single = match (only_tag.0.as_str(), only_tag.1.as_str()) {
            ("natural", "tree") => Some(NodeSingleInlined::Tree),
            ("power", "tower") => Some(NodeSingleInlined::PowerTower),
            ("power", "pole") => Some(NodeSingleInlined::PowerPole),
            ("amenity", "bench") => Some(NodeSingleInlined::Bench),
            ("emergency", "fire_hydrant") => Some(NodeSingleInlined::Hydrant),
            _ => None,
        };

        if tags_has_exactly(&tags, &[("natural", "tree"), ("leaf_type", "needleleaved")]) {
            try_single = Some(NodeSingleInlined::NeedleleavedTree)
        } else if tags_has_exactly(&tags, &[("natural", "tree"), ("leaf_type", "broadleaved")]) {
            try_single = Some(NodeSingleInlined::BroadleavedTree)
        }

        if let Some(t) = try_single {
            tags.clear();
            return NodeFields::Single(Some(t));
        }
    }

    let (fields, tags) = osm_tags_to_fields::fields::parse_tags_to_fields(tags);

    let mut combined_fields = Vec::with_capacity(fields.len() + tags.len());

    for t in fields {
        combined_fields.push(Field::Field(t));
    }
    for (k,v) in tags.iter() {
        combined_fields.push((k, v).into());
    }

    NodeFields::Multiple(Fields(combined_fields))
}

fn tags_has_exactly(tags: &osmpbfreader::Tags, expected: &[(&str, &str)]) -> bool {
    if tags.len() != expected.len() {
        return false;
    }

    for (k, v) in expected.iter() {
        if !tags.contains(&k, &v) {
            return false;
        }
    }

    return true;
}
