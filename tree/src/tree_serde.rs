use std::{
    collections::VecDeque,
    rc::Rc,
};

use btree_vec::BTreeVec;
use minimal_storage::{
    varint::{FromVarint, ToVarint},
    serialize_min::{DeserializeFromMinimal, SerializeMinimal},
    StorageReachable,
};

use super::{
    branch_id_creation,
    compare_by::OrderByFirst,
    tree_traits::{
        Dimension, MultidimensionalKey, MultidimensionalParent, MultidimensionalValue, Zero,
    },
    LongLatTree, RootTreeInfo, StoredTree,
};

impl<const DIMENSION_COUNT: usize, Key, Value> SerializeMinimal
    for LongLatTree<DIMENSION_COUNT, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
    for<'serialize> <Value as SerializeMinimal>::ExternalData<'serialize>: Copy,
{
    type ExternalData<'a> = <Value as SerializeMinimal>::ExternalData<'a>;

    fn minimally_serialize<'o, 's: 'o, W: std::io::Write>(
        &'o self,
        write_to: &mut W,
        external_data: Self::ExternalData<'s>,
    ) -> std::io::Result<()> {

        let header = (self.children.len() << 1) | (self.left_right_split.is_some() as usize);

        header.write_varint(write_to)?;

        if !self.children.is_empty() {
            let mut last_bbox = <Key::DeltaFromParent as Zero>::zero();

            for (bbox, child) in self.children.iter() {
                let offset = Key::delta_from_self(bbox, &last_bbox);

                offset.minimally_serialize(write_to, ())?;
                child.minimally_serialize(write_to, external_data)?;

                last_bbox = bbox.to_owned();
            }
        }

        Ok(())
    }
}

impl<const DIMENSION_COUNT: usize, Key, Value> DeserializeFromMinimal
    for LongLatTree<DIMENSION_COUNT, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
    for<'serialize> <Value as SerializeMinimal>::ExternalData<'serialize>: Copy,
{
    type ExternalData<'a> = &'a (RootTreeInfo, u64, Key::Parent);

    fn deserialize_minimal<'a, 'd: 'a, R: std::io::Read>(
        from: &'a mut R,
        external_data: Self::ExternalData<'d>,
    ) -> Result<Self, std::io::Error> {
        let (ref root_tree_info, id, bbox) = external_data;

        let header = usize::from_varint(from)?;

        let child_len = header >> 1;
        let has_left_right = (header & 1) == 1;

        let axis_index = (u64::BITS - id.leading_zeros()).checked_sub(1).unwrap() as usize % DIMENSION_COUNT;

        let axis =
            <Key::Parent as MultidimensionalParent<DIMENSION_COUNT>>::DimensionEnum::from_index(
                axis_index,
            );

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

        let left_right_split = if has_left_right {
            let (left_path, left_id) = branch_id_creation(&root_tree_info, *id, 0);
            let (right_path, right_id) = branch_id_creation(&root_tree_info, *id, 1);

            let (left_bbox, right_bbox) = bbox.split_evenly_on_dimension(&axis);

            let left_data = (Rc::clone(&root_tree_info), left_id, left_bbox);
            let right_data = (Rc::clone(&root_tree_info), right_id, right_bbox);

            Some((
                StoredTree::<DIMENSION_COUNT, Key, Value>::open(left_path, left_data),
                StoredTree::<DIMENSION_COUNT, Key, Value>::open(right_path, right_data),
            ))
        } else {
            None
        };

        Ok(Self {
            root_tree_info: Rc::clone(&external_data.0),
            bbox: bbox.clone(),
            direction: axis,
            children,
            left_right_split,
            id: external_data.1,
        })
    }
}

impl<const DIMENSION_COUNT: usize, Key, Value> StorageReachable<(RootTreeInfo, u64, Key::Parent)>
    for LongLatTree<DIMENSION_COUNT, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
    for<'serialize> <Value as SerializeMinimal>::ExternalData<'serialize>: Copy,
{
    fn flush_children<'a>(
        &'a mut self,
        serialize_data: <Self as SerializeMinimal>::ExternalData<'a>,
    ) -> std::io::Result<()> {
        let mut stack = VecDeque::new();

        if let Some((l, r)) = &mut self.left_right_split {
            stack.push_back(l);
            stack.push_back(r);
        }

        while let Some(item) = stack.pop_front() {
            if let Some(result) = item.flush_without_children(serialize_data) {
                result?;
            }

            if let Some((l, r)) = &mut item.ref_mut().left_right_split {
                if l.is_dirty() {
                    stack.push_back(l);
                }

                if r.is_dirty() {
                    stack.push_back(r);
                }
            }
        }

        Ok(())
    }
}
