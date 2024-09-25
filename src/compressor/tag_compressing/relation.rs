use osmpbfreader::Tags;

use super::{InlinedTags, TagCollection};

#[derive(Clone, Debug)]
pub struct Relation {}


pub fn inline_relation_tags(mut tags: Tags) -> InlinedTags<Relation> {
    InlinedTags {
        inline: Relation {},
        other: tags.drain_to_literal_list(),
    }
}