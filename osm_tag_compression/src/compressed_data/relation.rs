use minimal_storage::{packed_string_serialization::is_final::IterIsFinal, pooled_storage::Pool, serialize_min::SerializeMinimal};
use osm_value_atom::LiteralValue;
use osmpbfreader::{OsmObj, Ref, Relation, RelationId};

use crate::{compressed_data::flattened_id, field::Field, removable::remove_non_stored_tags};

use tree::{bbox::BoundingBox, point_range::StoredBinaryTree};

use super::{unflattened_id, CompressedOsmData, Fields, FIND_GROUP_MAX_DIFFERENCE};

pub fn osm_relation_to_compressed_node<const C: usize>(mut relation: Relation, bbox_cache: &StoredBinaryTree<C, u64, BoundingBox<i32>>) -> Result<CompressedOsmData, OsmObj> {

    let mut bounding_boxes = Vec::new();
    let mut low = flattened_id(&relation.refs[0].member);
    for (is_last_child, child) in relation.refs.iter().is_final() {
        let id = flattened_id(&child.member);
        let stride = id - low;

        if stride >= FIND_GROUP_MAX_DIFFERENCE || is_last_child {
            low = id;
            bounding_boxes.extend(bbox_cache
                .find_entries_in_box(&(low..=id))
                .filter(|x| {
                    if relation.refs.iter().any(|c: &Ref| c.member == unflattened_id(x.0)) {
                        true
                    } else {
                        false
                    }
                })
                .map(|x| (x.1)));
        }
    }

    if bounding_boxes.len() != relation.refs.len() {
        return Err(OsmObj::Relation(relation));
    }

    let bbox = bounding_boxes.into_iter().collect();

    

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
    pools: &mut (Pool<Field>, Pool<LiteralValue>),
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
    let literals = &tags.0;

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