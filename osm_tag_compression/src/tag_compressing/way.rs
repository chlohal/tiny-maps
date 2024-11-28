use osmpbfreader::Tags;

use super::{InlinedTags, TagCollection};

#[derive(Clone, Debug)]
pub struct Way {}


pub fn inline_way_tags(mut tags: Tags) -> InlinedTags<Way> {
    InlinedTags {
        inline: Way {},
        other: tags.drain_to_literal_list(),
    }
}