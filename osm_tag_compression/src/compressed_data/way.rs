use minimal_storage::{
    pooled_storage::Pool,
    serialize_min::{DeserializeFromMinimal, SerializeMinimal},
};
use osm_value_atom::LiteralValue;
use osmpbfreader::{OsmId, Way, WayId};

use crate::{compressed_data::flattened_id, field::Field, removable::remove_non_stored_tags};

use tree::{bbox::BoundingBox, point_range::StoredBinaryTree};

use super::{CompressedOsmData, Fields};

pub fn osm_way_to_compressed_node<const C: usize>(
    mut way: Way,
    bbox_cache: &StoredBinaryTree<C, u64, BoundingBox<i32>>,
) -> Result<CompressedOsmData, Way> {
    let mut children = Vec::with_capacity(way.nodes.len());

    let bbox: Option<BoundingBox<i32>> = way
        .nodes
        .iter()
        .map(|node| bbox_cache.get_owned(&flattened_id(&OsmId::Node(*node))))
        .map(|x| {
            if let Some(x) = x {
                children.push((*x.x(), *x.y()));
            }
            x
        })
        .collect();

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
    pool: &Pool<LiteralValue>,
    id: &WayId,
    tags: &Fields,
    children: &Vec<(i32, i32)>,
    bbox: &BoundingBox<i32>,
) -> Result<(), std::io::Error> {
    //first byte layout:
    //0: not a node
    //1: yes a way
    //others: todo
    let header = 0b01_00_0000u8;

    write_to.write_all(&[header])?;

    id.0.minimally_serialize(write_to, ())?;

    //chuck the nodes into the buffer directly (by position)
    let self_x = *bbox.x();
    let self_y = *bbox.y();
    children.len().minimally_serialize(write_to, ())?;
    for child in children.iter() {
        let x_diff = i32::abs_diff(self_x, child.0);
        let y_diff = i32::abs_diff(self_y, child.1);
        x_diff.minimally_serialize(write_to, ())?;
        y_diff.minimally_serialize(write_to, ())?;
    }

    //and just chuck all the literals into the literal pool and then put em at the end.
    let literals = &tags.0;

    literals.len().minimally_serialize(write_to, ())?;
    for literal in literals.iter() {
        literal.minimally_serialize(write_to, pool)?;
    }

    Ok(())
}

pub fn deserialize_way(
    from: &mut impl std::io::Read,
    bbox: &BoundingBox<i32>,
    pool: &mut Pool<LiteralValue>,
) -> std::io::Result<(WayId, Vec<(i32, i32)>, Vec<Field>)> {
    let header = u8::deserialize_minimal(from, ())?;

    if header != 0b01_00_0000u8 {
        return Err(std::io::ErrorKind::InvalidData.into());
    }

    let id = WayId(DeserializeFromMinimal::deserialize_minimal(from, ())?);

    let points_count = usize::deserialize_minimal(from, ())?;

    let mut points = Vec::with_capacity(points_count);

    let base_x = *bbox.x();
    let base_y = *bbox.y();

    for _ in 0..points_count {
        let x_off = u32::deserialize_minimal(from, ())?;
        let y_off = u32::deserialize_minimal(from, ())?;

        let x = base_x.wrapping_add_unsigned(x_off);
        let y = base_y.wrapping_add_unsigned(y_off);

        points.push((x, y));
    }

    let fields_count = usize::deserialize_minimal(from, ())?;

    let mut fields = Vec::with_capacity(fields_count);

    for _ in 0..fields_count {
        fields.push(DeserializeFromMinimal::deserialize_minimal(from, &mut *pool)?)
    }

    Ok((id, points, fields))
}

pub fn get_points(
    from: &mut impl std::io::Read,
    bbox: &BoundingBox<i32>,
) -> std::io::Result<Vec<(i32, i32)>> {
    let header = u8::deserialize_minimal(from, ())?;

    if header != 0b01_00_0000u8 {
        return Err(std::io::ErrorKind::InvalidData.into());
    }

    let _id = WayId(DeserializeFromMinimal::deserialize_minimal(from, ())?);

    let len = usize::deserialize_minimal(from, ())?;

    let mut vec = Vec::with_capacity(len);

    let base_x = *bbox.x();
    let base_y = *bbox.y();

    for _ in 0..len {
        let x_off = u32::deserialize_minimal(from, ())?;
        let y_off = u32::deserialize_minimal(from, ())?;

        let x = base_x.wrapping_add_unsigned(x_off);
        let y = base_y.wrapping_add_unsigned(y_off);

        vec.push((x, y));
    }

    Ok(vec)
}
