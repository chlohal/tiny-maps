use std::{path::PathBuf, sync::{atomic::Ordering::{AcqRel, Acquire}, OnceLock}};

use btree_vec::BTreeVec;
use minimal_storage::{
    paged_storage::PageId,
    serialize_min::{DeserializeFromMinimal, SerializeMinimal},
    varint::ToVarint,
    StorageReachable,
};

use crate::{
    sparse::{structure::Inner, tree::split_id},
    tree_traits::{
        Dimension, MultidimensionalParent
    },
};

use super::{SparseKey, SparseValue};

impl<const DIMENSION_COUNT: usize, const NODE_SATURATION_POINT: usize, Key, Value> SerializeMinimal
    for Inner<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>
where
    Key: SparseKey<DIMENSION_COUNT>,
    Value: SparseValue,
{
    type ExternalData<'s> = ();

    fn minimally_serialize<'a, 's: 'a, W: std::io::Write>(
        &'a self,
        write_to: &mut W,
        external_data: Self::ExternalData<'s>,
    ) -> std::io::Result<()> {
        self.children.len().write_varint(write_to)?;

        for (bbox, child) in self.children.iter() {
            bbox.fast_minimally_serialize(write_to, ())?;
            child.fast_minimally_serialize(write_to, external_data)?;
        }

        Ok(())
    }
}

impl<const DIMENSION_COUNT: usize, const NODE_SATURATION_POINT: usize, Key, Value>
    DeserializeFromMinimal for Inner<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>
where
    Key: SparseKey<DIMENSION_COUNT>,
    Value: SparseValue,
{
    type ExternalData<'d> = ();

    fn deserialize_minimal<'a, 'd: 'a, R: std::io::Read>(
        from: &'a mut R,
        _external_data: Self::ExternalData<'d>,
    ) -> Result<Self, std::io::Error> {
        let child_len = usize::deserialize_minimal(from, ())?;

        let mut children = BTreeVec::with_capacity(child_len);
        for _ in 0..child_len {
            let key = Key::fast_deserialize_minimal(from, ())?;
            let item = Value::fast_deserialize_minimal(from, ())?;

            children.push(key, item);
        }

        Ok(Self { children })
    }
}

impl<const DIMENSION_COUNT: usize, const NODE_SATURATION_POINT: usize, Key, Value>
    DeserializeFromMinimal
    for crate::sparse::structure::Root<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>
where
    Key: SparseKey<DIMENSION_COUNT>,
    Value: SparseValue,
{
    type ExternalData<'d> = ();

    fn deserialize_minimal<'a, 'd: 'a, R: std::io::Read>(
        from: &'a mut R,
        _external_data: Self::ExternalData<'d>,
    ) -> Result<Self, std::io::Error> {
        let root_bbox: Key::Parent = DeserializeFromMinimal::deserialize_minimal(from, ())?;

        let node = DeserializeFromMinimal::deserialize_minimal(
            from,
            (root_bbox.clone(), Default::default()),
        )?;

        Ok(Self { root_bbox, node })
    }
}

impl<const DIMENSION_COUNT: usize, const NODE_SATURATION_POINT: usize, Key, Value> SerializeMinimal
    for crate::sparse::structure::Root<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>
where
    Key: SparseKey<DIMENSION_COUNT>,
    Value: SparseValue,
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

impl<const DIMENSION_COUNT: usize, const NODE_SATURATION_POINT: usize, Key, Value>
    DeserializeFromMinimal
    for crate::sparse::structure::Node<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>
where
    Key: SparseKey<DIMENSION_COUNT>,
    Value: SparseValue,
{
    type ExternalData<'d> = (
        Key::Parent,
        <Key::Parent as MultidimensionalParent<DIMENSION_COUNT>>::DimensionEnum,
    );

    fn deserialize_minimal<'a, 'd: 'a, R: std::io::Read>(
        from: &'a mut R,
        (parent, direction): Self::ExternalData<'d>,
    ) -> Result<Self, std::io::Error> {
        let has_split = u8::deserialize_minimal(from, ())? == b'y';
        let page_id = PageId::deserialize_minimal(from, ())?;
        let child_count = usize::deserialize_minimal(from, ())?.into();

        let left_right_split = if has_split {

            let (left_bbox, right_bbox) = parent.split_evenly_on_dimension(&direction);
            let next_dir = direction.next_axis();

            OnceLock::from((
                Box::new(Self::deserialize_minimal(
                    from,
                    (left_bbox, next_dir),
                )?),
                Box::new(Self::deserialize_minimal(
                    from,
                    (right_bbox, next_dir),
                )?),
            ))
        } else {
            OnceLock::new()
        };

        Ok(Self {
            bbox: parent,
            page_id,
            child_count,
            left_right_split,
            __phantom: std::marker::PhantomData,
        })
    }
}

impl<const DIMENSION_COUNT: usize, const NODE_SATURATION_POINT: usize, Key, Value> SerializeMinimal
    for crate::sparse::structure::Node<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>
where
    Key: SparseKey<DIMENSION_COUNT>,
    Value: SparseValue,
{
    type ExternalData<'d> = ();

    fn minimally_serialize<'a, 's: 'a, W: std::io::Write>(
        &'a self,
        write_to: &mut W,
        external_data: Self::ExternalData<'s>,
    ) -> std::io::Result<()> {
        match &self.left_right_split.get() {
            Some((l, r)) => {
                (b'y').minimally_serialize(write_to, ())?;
                self.page_id.minimally_serialize(write_to, ())?;
                self.child_count.load(Acquire).minimally_serialize(write_to, ())?;

                l.minimally_serialize(write_to, external_data)?;
                r.minimally_serialize(write_to, external_data)?;
            }
            None => {
                (b'n').minimally_serialize(write_to, ())?;
                self.page_id.minimally_serialize(write_to, ())?;
                self.child_count.load(Acquire).minimally_serialize(write_to, ())?;
            }
        }

        Ok(())
    }
}
