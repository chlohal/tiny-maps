use osmpbfreader::Tags;

use osm_literals::{
    literal::{Literal, WellKnownKeyVar::Address},
    literal_value::LiteralValue,
    structured_elements::{address::OsmAddress, contact::OsmContactInfo},
};

use super::{remove_non_stored_tags, InlinedTags, TagCollection};

#[derive(Clone, Debug)]
pub struct Node {
    pub single: Option<NodeSingleInlined>,
    pub multiple: InlinedNodeTags,
}
impl Node {
    pub fn is_single(&self) -> bool {
        self.multiple.is_none()
    }
}

#[derive(Clone, Debug)]
pub struct InlinedNodeTags {
    pub address: Option<Literal>,
    pub public_transit: Option<Literal>,
    pub shop: Option<Literal>,
    pub name: Option<LiteralValue>,

    pub contact: Option<Literal>,
    pub place: Option<LiteralValue>,
    pub operator: Option<LiteralValue>,
    pub additional: Vec<Literal>,
}
impl InlinedNodeTags {
    fn is_none(&self) -> bool {
        self.additional.is_empty()
            && self.address.is_none()
            && self.public_transit.is_none()
            && self.shop.is_none()
            && self.name.is_none()
            && self.contact.is_none()
            && self.operator.is_none()
    }

    fn none() -> InlinedNodeTags {
        InlinedNodeTags {
            address: None,
            public_transit: None,
            shop: None,
            name: None,
            contact: None,
            place: None,
            operator: None,
            additional: vec![],
        }
    }
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

pub fn inline_node_tags(mut tags: Tags) -> InlinedTags<Node> {
    remove_non_stored_tags(&mut tags);

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
            try_single = Some(NodeSingleInlined::BroadleavedTree)
        }

        if let Some(t) = try_single {
            tags.clear();

            return InlinedTags {
                inline: Node {
                    single: Some(t),
                    multiple: InlinedNodeTags::none(),
                },
                other: Vec::new(),
            };
        }
    }

    let mut additional = Vec::new();

    let address = OsmAddress::make_from_tags(&mut tags, "addr");

    for addr_count in 2.. {
        let address = OsmAddress::make_from_tags(&mut tags, format!("addr{addr_count}").as_str());

        if address.is_none() {
            break;
        }

        additional.push(Literal::WellKnownKeyVar(Address(address)));
    }

    //add contact address, if it's encoded differently.
    //this will almost always be a karlsruhle minimal address
    //(and usually in France)
    additional.extend(
        OsmAddress::make_from_tags(&mut tags, "contact")
            .as_option()
            .map(Into::into),
    );

    let (contact, other_contact) = {
        let contact_no_prefix = OsmContactInfo::make_from_tags(&mut tags, "").as_option();
        let contact_prefix = OsmContactInfo::make_from_tags(&mut tags, "contact:").as_option();

        match (contact_no_prefix, contact_prefix) {
            (None, None) => (None, None),
            (None, Some(c)) => (Some(c), None),
            (Some(c), None) => (Some(c), None),
            (Some(c1), Some(c2)) => (Some(c1), Some(c2)),
        }
    };

    additional.extend(other_contact.into_iter().map(Into::into));

    return InlinedTags {
        inline: Node {
            multiple: InlinedNodeTags {
                address: address.as_option().map(Into::into),
                public_transit: None,
                shop: None,
                name: None,
                contact: contact.map(Into::into),
                place: None,
                operator: None,
                additional,
            },
            single: None,
        },
        other: tags.drain_to_literal_list(),
    };

    //todo: implement ability to use multiple & single at same time
}
