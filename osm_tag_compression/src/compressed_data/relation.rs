use minimal_storage::{pooled_storage::Pool, serialize_min::SerializeMinimal};
use osm_value_atom::LiteralValue;
use osmpbfreader::{OsmObj, Ref, Relation, RelationId};

use crate::{compressed_data::flattened_id, field::Field, tag_compressing::{self, relation::inline_relation_tags, InlinedTags}};

use tree::{bbox::BoundingBox, point_range::StoredBinaryTree};

use super::CompressedOsmData;

pub fn osm_relation_to_compressed_node<const C: usize>(relation: Relation, bbox_cache: &mut StoredBinaryTree<C, u64, BoundingBox<i32>>) -> Result<CompressedOsmData, OsmObj> {

    let bbox: Option<BoundingBox<i32>> = relation.refs.iter().map(|child| {
        let id = flattened_id(&child.member);
        
        let child_box = bbox_cache.find_first_item_at_key_exact(&id)?.into_inner();

        Some(child_box.0)
    }).collect();

    let Some(bbox) = bbox else { return Err(OsmObj::Relation(relation)) };

    let tags = inline_relation_tags(relation.tags);

    Ok(CompressedOsmData::Relation { bbox, tags, id: relation.id, refs: relation.refs })
}

pub fn serialize_relation<W: std::io::Write>(
    write_to: &mut W,
    pools: &mut (Pool<Field>, Pool<LiteralValue>),
    id: &RelationId,
    tags: &InlinedTags<tag_compressing::relation::Relation>,
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
    let literals = &tags.other;

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