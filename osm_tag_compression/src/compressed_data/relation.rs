use minimal_storage::{packed_string_serialization::is_final::IterIsFinal, pooled_storage::Pool, serialize_min::SerializeMinimal};
use osm_value_atom::LiteralValue;
use osmpbfreader::{OsmObj, Ref, Relation, RelationId};

use crate::{compressed_data::flattened_id, field::Field, removable::remove_non_stored_tags};

use tree::{bbox::BoundingBox, point_range::StoredBinaryTree};

use super::{CompressedOsmData, Fields};

pub fn osm_relation_to_compressed_node<const C: usize>(mut relation: Relation, bbox_cache: &StoredBinaryTree<C, u64, BoundingBox<i32>>) -> Result<CompressedOsmData, OsmObj> {
    let bbox: Option<BoundingBox<i32>> = relation.refs.iter().map(|r| {
        let id = flattened_id(&r.member);
        bbox_cache.get_owned(&id)
    }).collect();

    let Some(bbox) = bbox else {
        return Err(OsmObj::Relation(relation));
    };

    remove_non_stored_tags(&mut relation.tags);

    let (fields, tags) = osm_tags_to_fields::fields::parse_tags_to_fields(relation.tags);

    let mut combined_fields = Vec::with_capacity(fields.len() + tags.len());

    for t in fields {
        combined_fields.push(Field::Field(t));
    }
    for (k,v) in tags.iter() {
        combined_fields.push((k, v).into());
    }

    Ok(CompressedOsmData::Relation { bbox, tags: Fields(combined_fields), id: relation.id, refs: relation.refs })
}

pub fn serialize_relation<W: std::io::Write>(
    write_to: &mut W,
    pools: &(Pool<Field>, Pool<LiteralValue>),
    id: &RelationId,
    tags: &Fields,
    children: &Vec<Ref>
) -> Result<(), std::io::Error> {
    
    //first byte layout:
    //0: not a node
    //0: not a way
    //others: todo
    let header = 0b00_00_0000u8;

    write_to.write_all(&[header])?;

    id.0.minimally_serialize(write_to, ())?;

    //and just chuck all the literals into the literal pool and then put em at the end.
    let Fields(literals) = &tags;

    literals.len().minimally_serialize(write_to, ())?;
    for literal in literals.iter() {
        let (p1, p2) = pools;
        let id = Pool::<Field>::insert(p1, literal, p2)?;

        id.minimally_serialize(write_to, ())?;
    }

    //same with the children
    children.len().minimally_serialize(write_to, ())?;
    
    let mut buffer = Vec::new();
    for child in children.iter() {
        child.role.as_str().minimally_serialize(write_to, 0.into())?;

        flattened_id(&child.member).minimally_serialize(&mut buffer, ())?;
    }

    write_to.write_all(&buffer)?;
    

    Ok(())
}