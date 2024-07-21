use std::collections::BTreeMap;

use osmpbfreader::Tags;

use crate::compressor::literals::{packed_strings::PackedString, structured_elements::{address::OsmAddress, public_transit::OsmPublicTransit, shop_amenity::OsmShopAmenity}};

use super::{InlinedTags, TagCollection};

pub enum Node {
    None,
    Single(NodeSingleInlined),
    Multiple(Option<NodeSingleInlined>, InlinedNodeTags),
}
impl Node {
    pub fn is_single(&self) -> bool {
        match self {
            Node::Single(_) => true,
            _ => false,
        }
    }

    pub fn is_multiple(&self) -> bool {
        match self {
            Node::Multiple(_, _) => true,
            _ => false,
        }
    }
}

pub struct InlinedNodeTags {
    pub address: Option<OsmAddress>,
    pub public_transit: Option<OsmPublicTransit>,
    pub shop: Option<OsmShopAmenity>,
    pub name: Option<PackedString>,
    pub highway: Option<PackedString>,
    pub place: Option<PackedString>,
    pub operator: Option<PackedString>
}

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

pub fn inline_node_tags<'a>(tags: &'a mut Tags) -> InlinedTags<Node> {
    tags.remove("source");

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

        if tags.has_exactly(&[("natural", "tree"), ("leaf_type", "needleleaved")]) {
            try_single = Some(NodeSingleInlined::NeedleleavedTree)
        } else if tags.has_exactly(&[("natural", "tree"), ("leaf_type", "broadleaved")]) {
            try_single = Some(NodeSingleInlined::NeedleleavedTree)
        }

        if let Some(t) = try_single {
            tags.clear();

            return InlinedTags {
                inline: Node::Single(t),
                other: BTreeMap::new(),
            };
        }
    }

    

    return InlinedTags {
        inline: Node::None,
        other: tags.drain_to_literal_map(),
    };
}
