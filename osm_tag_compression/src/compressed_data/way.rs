use minimal_storage::{
    packed_string_serialization::is_final::IterIsFinal, pooled_storage::Pool,
    serialize_min::SerializeMinimal,
};
use osm_value_atom::LiteralValue;
use osmpbfreader::{OsmId, Way, WayId};

use crate::{compressed_data::flattened_id, field::Field, removable::remove_non_stored_tags};

use tree::{bbox::BoundingBox, point_range::StoredBinaryTree};

use super::{unflattened_id, CompressedOsmData, Fields, FIND_GROUP_MAX_DIFFERENCE};

pub fn osm_way_to_compressed_node<const C: usize>(
    mut way: Way,
    bbox_cache: &StoredBinaryTree<C, u64, BoundingBox<i32>>,
) -> Result<CompressedOsmData, Way> {
    let mut children = Vec::with_capacity(way.nodes.len());

    let bbox: Option<BoundingBox<i32>> = way.nodes.iter().map(|node| {
        let id = flattened_id(&OsmId::Node(*node));
        children.push(id);
        bbox_cache.find_first_item_at_key_exact(&id)
    }).collect();


    let Some(bbox) = bbox else {
        return Err(way);
    };

    remove_non_stored_tags(&mut way.tags);

    let (fields, tags) = osm_tags_to_fields::fields::parse_tags_to_fields(way.tags);

    let mut combined_fields = Vec::with_capacity(fields.len() + tags.len());

    for t in fields {
        combined_fields.push(Field::Field(t));
    }
    for (k, v) in tags.iter() {
        combined_fields.push((k, v).into());
    }

    Ok(CompressedOsmData::Way {
        bbox,
        tags: super::Fields(combined_fields),
        id: way.id,
        children,
    })
}

pub fn serialize_way<W: std::io::Write>(
    write_to: &mut W,
    pools: &mut (Pool<Field>, Pool<LiteralValue>),
    id: &WayId,
    tags: &Fields,
    children: &Vec<u64>,
) -> Result<(), std::io::Error> {
    //first byte layout:
    //0: not a node
    //1: yes a way
    //others: todo
    let header = 0b01_00_0000u8;

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

    //same with the nodeIds
    children.len().minimally_serialize(write_to, ())?;
    for child in children.iter() {
        child.minimally_serialize(write_to, ())?;
    }

    Ok(())
}
