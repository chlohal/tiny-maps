use std::path::PathBuf;

use btree_vec::BTreeVec;
use minimal_storage::{
    serialize_min::{DeserializeFromMinimal, SerializeMinimal},
    varint::ToVarint,
    Storage, StorageReachable,
};

use crate::{
    make_path, split_id,
    structure::Inner,
    tree_traits::{
        Dimension, MultidimensionalKey, MultidimensionalParent, MultidimensionalValue, Zero,
    },
};

impl<const DIMENSION_COUNT: usize, Key, Value> SerializeMinimal
    for crate::structure::Inner<DIMENSION_COUNT, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
    for<'serialize> <Value as SerializeMinimal>::ExternalData<'serialize>: Copy,
{
    type ExternalData<'s> = <Value as SerializeMinimal>::ExternalData<'s>;

    fn minimally_serialize<'a, 's: 'a, W: std::io::Write>(
        &'a self,
        write_to: &mut W,
        external_data: Self::ExternalData<'s>,
    ) -> std::io::Result<()> {
        self.children.len().write_varint(write_to)?;

        let mut last_bbox = <Key::DeltaFromParent as Zero>::zero();
        for (bbox, child) in self.children.iter() {
            let offset = Key::delta_from_self(bbox, &last_bbox);

            offset.minimally_serialize(write_to, ())?;
            child.minimally_serialize(write_to, external_data)?;

            last_bbox = bbox.to_owned();
        }

        Ok(())
    }
}

impl<const DIMENSION_COUNT: usize, Key, Value> DeserializeFromMinimal
    for crate::structure::Inner<DIMENSION_COUNT, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
    for<'serialize> <Value as SerializeMinimal>::ExternalData<'serialize>: Copy,
{
    type ExternalData<'d> = &'d <Key as MultidimensionalKey<DIMENSION_COUNT>>::Parent;

    fn deserialize_minimal<'a, 'd: 'a, R: std::io::Read>(
        from: &'a mut R,
        bbox: Self::ExternalData<'d>,
    ) -> Result<Self, std::io::Error> {
        let child_len = usize::deserialize_minimal(from, ())?;

        let mut children = BTreeVec::with_capacity(child_len);

        let mut last_bbox = Key::DeltaFromParent::zero();

        for _ in 0..child_len {
            let delt_delt_bbox = Key::DeltaFromSelf::deserialize_minimal(from, ())?;
            let delt_bbox = Key::apply_delta_from_self(&delt_delt_bbox, &last_bbox);
            let abs_bbox = Key::apply_delta_from_parent(&delt_bbox, bbox);

            last_bbox = delt_bbox;

            let item = Value::deserialize_minimal(from, &abs_bbox)?;

            children.push(delt_bbox, item);
        }

        Ok(Self { children })
    }
}

impl<const DIMENSION_COUNT: usize, Key, Value>
    StorageReachable<<Key as MultidimensionalKey<DIMENSION_COUNT>>::Parent>
    for crate::structure::Inner<DIMENSION_COUNT, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
    for<'serialize> <Value as SerializeMinimal>::ExternalData<'serialize>: Copy,
{
}

impl<const DIMENSION_COUNT: usize, Key, Value> DeserializeFromMinimal
    for crate::structure::Root<DIMENSION_COUNT, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
    for<'serialize> <Value as SerializeMinimal>::ExternalData<'serialize>: Copy,
{
    type ExternalData<'d> = &'d PathBuf;

    fn deserialize_minimal<'a, 'd: 'a, R: std::io::Read>(
        from: &'a mut R,
        external_data: Self::ExternalData<'d>,
    ) -> Result<Self, std::io::Error> {
        let root_bbox: Key::Parent = DeserializeFromMinimal::deserialize_minimal(from, ())?;

        let node = DeserializeFromMinimal::deserialize_minimal(
            from,
            (external_data, 1, root_bbox.clone(), Default::default()),
        )?;

        Ok(Self { root_bbox, node })
    }
}

impl<const DIMENSION_COUNT: usize, Key, Value> SerializeMinimal
    for crate::structure::Root<DIMENSION_COUNT, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
    for<'serialize> <Value as SerializeMinimal>::ExternalData<'serialize>: Copy,
{
    type ExternalData<'d> = ();

    fn minimally_serialize<'a, 's: 'a, W: std::io::Write>(
        &'a self,
        write_to: &mut W,
        _external_data: Self::ExternalData<'s>,
    ) -> std::io::Result<()> {
        self.root_bbox.minimally_serialize(write_to, ())?;

        self.node.minimally_serialize(write_to, ())?;

        Ok(())
    }
}

impl<const DIMENSION_COUNT: usize, Key, Value> DeserializeFromMinimal
    for crate::structure::Node<DIMENSION_COUNT, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
    for<'serialize> <Value as SerializeMinimal>::ExternalData<'serialize>: Copy,
{
    type ExternalData<'d> = (
        &'d PathBuf,
        u64,
        Key::Parent,
        <Key::Parent as MultidimensionalParent<DIMENSION_COUNT>>::DimensionEnum,
    );

    fn deserialize_minimal<'a, 'd: 'a, R: std::io::Read>(
        from: &'a mut R,
        (root_path, id, parent, direction): Self::ExternalData<'d>,
    ) -> Result<Self, std::io::Error> {
        let has_split = u8::deserialize_minimal(from, ())? != 0;

        let left_right_split = if has_split {
            let (left_id, right_id) = split_id(id);

            let (left_bbox, right_bbox) = parent.split_evenly_on_dimension(&direction);
            let next_dir = direction.next_axis();

            Some((
                Box::new(Self::deserialize_minimal(
                    from,
                    (root_path, left_id, left_bbox, next_dir),
                )?),
                Box::new(Self::deserialize_minimal(
                    from,
                    (root_path, right_id, right_bbox, next_dir),
                )?),
            ))
        } else {
            None
        };

        let path = make_path(root_path, id);

        Ok(Self {
            bbox: parent.clone(),
            values: Storage::<
                <Key as MultidimensionalKey<DIMENSION_COUNT>>::Parent,
                Inner<DIMENSION_COUNT, Key, Value>,
            >::open(path, parent),
            left_right_split,
            id,
        })
    }
}

impl<const DIMENSION_COUNT: usize, Key, Value> SerializeMinimal
    for crate::structure::Node<DIMENSION_COUNT, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
    for<'serialize> <Value as SerializeMinimal>::ExternalData<'serialize>: Copy,
{
    type ExternalData<'d> = ();

    fn minimally_serialize<'a, 's: 'a, W: std::io::Write>(
        &'a self,
        write_to: &mut W,
        external_data: Self::ExternalData<'s>,
    ) -> std::io::Result<()> {
        match &self.left_right_split {
            Some((l, r)) => {
                (1u8).minimally_serialize(write_to, ())?;

                l.minimally_serialize(write_to, external_data)?;
                r.minimally_serialize(write_to, external_data)?;
            }
            None => {
                (0u8).minimally_serialize(write_to, ())?;
            }
        }

        Ok(())
    }
}
