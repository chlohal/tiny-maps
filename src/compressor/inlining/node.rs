use osmpbfreader::Tags;

use super::InlinedTags;


pub enum Node {
    None,
    Single(NodeSingleInlined),
    Multiple(Tags)
}

#[repr(u8)]
pub enum NodeSingleInlined {
    Tree = 1, PowerTower = 2, PowerPole = 3, Entrance = 4, Bench = 5, Hydrant = 6, Gate = 7
}

pub fn inline_node_tags<'a>(tags: &'a mut Tags) -> InlinedTags<'a, Node> {

    if tags.len() == 1 {
        let only_tag = tags.iter().next().unwrap();

        let try_single = match (only_tag.0.as_str(), only_tag.1.as_str()) {
            ("natural", "tree") => Some(NodeSingleInlined::Tree),
            ("power", "tower") => Some(NodeSingleInlined::PowerTower),
            ("power", "pole") => Some(NodeSingleInlined::PowerPole),
            ("entrance", "yes") => Some(NodeSingleInlined::Entrance),
            ("amenity", "bench") => Some(NodeSingleInlined::Bench),
            ("emergency", "fire_hydrant") => Some(NodeSingleInlined::Hydrant),
            ("barrier", "gate") => Some(NodeSingleInlined::Gate),
            _ => None
        };

        if let Some(t) = try_single {
            tags.clear();

            return InlinedTags {
                inline: Node::Single(t),
                other: tags,
            }
        }
    }

    return InlinedTags { inline: Node::None, other: tags }


}
