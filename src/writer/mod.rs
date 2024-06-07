use std::{io::Write, ops::Deref};

use osmpbfreader::{Node, OsmObj, Relation, Tags, Way};

use self::{
    literals::LiteralPool,
    types::{ElementType, KnownRelationTypeTag},
};

mod literals;
mod macros;
mod tags;
mod types;

pub fn write_element(
    element: &OsmObj,
    dest: &mut impl Write,
    literals: &mut LiteralPool,
) -> Result<usize, std::io::Error> {
    match element {
        OsmObj::Node(node) => write_node(node, dest, literals),
        OsmObj::Way(way) => write_way(way, dest, literals),
        OsmObj::Relation(relation) => write_relation(relation, dest, literals),
    }
}

pub fn write_node(
    node: &Node,
    dest: &mut impl Write,
    literals: &mut LiteralPool,
) -> Result<usize, std::io::Error> {
    let way_type = ElementType::Node;
    todo!();
}

pub fn write_way(
    way: &Way,
    dest: &mut impl Write,
    literals: &mut LiteralPool,
) -> Result<usize, std::io::Error> {
    let way_type = ElementType::Way;

    todo!();
}

pub fn write_relation(
    relation: &Relation,
    dest: &mut impl Write,
    literals: &mut LiteralPool,
) -> Result<usize, std::io::Error> {
    match KnownRelationTypeTag::try_match(&relation.tags) {
        Some(t) => write_typed_relation(relation, t, dest, literals),
        None => write_untyped_relation(relation, dest, literals),
    }
}

fn write_typed_relation(
    relation: &Relation,
    rel_type: KnownRelationTypeTag,
    dest: &mut impl Write,
    literals: &mut LiteralPool,
) -> Result<usize, std::io::Error> {
    let broad_discrim = ElementType::RelationTyped.discriminant();

    let has_roles = relation.refs.iter().any(|x| x.role != "");

    let type_byte =
        (broad_discrim << 6) | (rel_type.discriminant() << 1) | (if has_roles { 1 } else { 0 });

    dest.write(&[type_byte]);

    for rel_ref in relation.refs.iter() {
        if has_roles {
            dest.write(rel_ref.member.inner_id().to_le_bytes().as_slice())?;
            dest.write(literals.get_id(&*rel_ref.role).to_le_bytes().as_slice())?;
        } else {
            dest.write(rel_ref.member.inner_id().to_le_bytes().as_slice())?;
        }
    }

    Ok(0)
}

fn write_untyped_relation(
    relation: &Relation,
    dest: &mut impl Write,
    literals: &mut LiteralPool,
) -> Result<usize, std::io::Error> {
    todo!();
}

pub fn write_tags(tags: &Tags, dest: &mut impl Write) {
    for (k, v) in tags.iter() {
        write_tag_name(k);
    }
}

pub fn write_tag_name(name: &impl Deref<Target = str>) {}
