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
    let mut real_children = Vec::with_capacity(way.nodes.len());
    let mut points = Vec::with_capacity(way.nodes.len());

    let mut low = flattened_id(&OsmId::Node(way.nodes[0]));

    // for child in way.nodes.iter() {
    //     points.push(
    //         bbox_cache
    //             .find_first_item_at_key_exact(&flattened_id(&OsmId::Node(*child)))
    //             .unwrap(),
    //     )
    // }

    for (is_last_child, child) in way.nodes.iter().is_final() {
        let id = flattened_id(&OsmId::Node(*child));
        let stride = id - low;

        if stride >= FIND_GROUP_MAX_DIFFERENCE || is_last_child {
            low = id;
            points.extend(bbox_cache
                .find_entries_in_box(&(low..=id))
                .filter(|x| {
                    if unflattened_id(x.0).node().is_some_and(|id| way.nodes.contains(&id)) {
                        real_children.push(id);
                        true
                    } else {
                        false
                    }
                })
                .map(|x| (*x.1.x(), *x.1.y())));
        }
    }

    if points.len() != way.nodes.len() {
        return Err(way);
    }

    let bbox = BoundingBox::<i32>::from_iter(points.into_iter());

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
        children: real_children,
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
