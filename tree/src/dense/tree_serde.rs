use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{atomic::AtomicUsize, Mutex, OnceLock, RwLock},
};

use btree_vec::BTreeVec;
use minimal_storage::{
    paged_storage::{FastNullablePageId, PageId},
    serialize_min::{DeserializeFromMinimal, SerializeMinimal},
    varint::ToVarint,
    StorageReachable,
};

use crate::{
    dense::{
        structure::{ExternalChildrenCount, Inner},
        tree::split_id,
    },
    tree_traits::{
        Dimension, MultidimensionalKey, MultidimensionalParent, MultidimensionalValue, Zero,
    },
    PAGE_SIZE,
};

impl<const DIMENSION_COUNT: usize, const NODE_SATURATION_POINT: usize, Key, Value> SerializeMinimal
    for Inner<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
{
    type ExternalData<'s> = ();

    fn minimally_serialize<'a, 's: 'a, W: std::io::Write>(
        &'a self,
        write_to: &mut W,
        external_data: Self::ExternalData<'s>,
    ) -> std::io::Result<()> {
        self.children.len().minimally_serialize(write_to, ())?;

        if self.children.len() == 0 {
            return Ok(());
        }

        let mut last_bbox = <Key::DeltaFromParent as Zero>::zero();

        let mut actual_count = 0;

        for (bbox, child) in self.children.iter() {
            debug_assert!(*bbox >= last_bbox);

            let offset = Key::delta_from_self(bbox, &last_bbox);

            offset.minimally_serialize(write_to, ())?;
            child.minimally_serialize(write_to, external_data)?;

            last_bbox = bbox.to_owned();
            actual_count += 1;
        }

        Ok(())
    }
}

impl<const DIMENSION_COUNT: usize, const NODE_SATURATION_POINT: usize, Key, Value>
    DeserializeFromMinimal for Inner<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
{
    type ExternalData<'d> = (
        &'d PageId<{ PAGE_SIZE }>,
        &'d ExternalChildrenCount<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>,
        &'d <Key as MultidimensionalKey<DIMENSION_COUNT>>::Parent,
    );

    fn deserialize_minimal<'a, 'd: 'a, R: std::io::Read>(
        from: &'a mut R,
        (page_id_borrow, child_len, bbox): Self::ExternalData<'d>,
    ) -> Result<Self, std::io::Error> {
        //okay to load this lazily: since its being
        //loaded currently it won't be modified until it's done loading
        let child_len = child_len.get_initial(&page_id_borrow);

        let child_len_check = usize::deserialize_minimal(from, ())?;

        debug_assert_eq!(child_len, child_len_check);

        let mut last_bbox = Key::DeltaFromParent::zero();

        let children_sorted_deque = (0..child_len).map(|_| {
            let delt_delt_bbox = Key::DeltaFromSelfAsChild::deserialize_minimal(from, ())?;

            let delt_bbox = Key::apply_delta_from_self(&delt_delt_bbox, &last_bbox);
            let abs_bbox = Key::apply_delta_from_parent(&delt_bbox, bbox);

            last_bbox = delt_bbox;

            let item = Value::deserialize_minimal(from, &abs_bbox)?;

            Ok((delt_bbox, item))
        });

        let children: std::io::Result<
            BTreeVec<<Key as MultidimensionalKey<DIMENSION_COUNT>>::DeltaFromParent, Value>,
        > = unsafe { BTreeVec::from_sorted_iter_failable(children_sorted_deque) };
        let children = children?;

        Ok(Self { children })
    }
}

impl<const DIMENSION_COUNT: usize, const NODE_SATURATION_POINT: usize, Key, Value>
    DeserializeFromMinimal
    for crate::dense::structure::Root<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
{
    type ExternalData<'d> = &'d PathBuf;

    fn deserialize_minimal<'a, 'd: 'a, R: std::io::Read>(
        from: &'a mut R,
        external_data: Self::ExternalData<'d>,
    ) -> Result<Self, std::io::Error> {
        let root_bbox: Key::Parent = DeserializeFromMinimal::deserialize_minimal(from, ())?;

        let node = DeserializeFromMinimal::deserialize_minimal(
            from,
            (external_data, 1, root_bbox.clone(), Dimension::arbitrary_first()),
        )?;

        Ok(Self { root_bbox, node })
    }
}

impl<const DIMENSION_COUNT: usize, const NODE_SATURATION_POINT: usize, Key, Value> SerializeMinimal
    for crate::dense::structure::Root<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
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
    for crate::dense::structure::Node<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
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
        let page_id = FastNullablePageId::deserialize_minimal(from, ())?;
        let children_count = usize::deserialize_minimal(from, ())?.into();
        let has_split = u8::deserialize_minimal(from, ())? != 0;

        let left_right_split = if has_split {
            let (left_id, right_id) = split_id(id);

            let (left_bbox, right_bbox) = parent.split_evenly_on_dimension(&direction);
            let next_dir = direction.next_axis();

            OnceLock::from((
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
            OnceLock::new()
        };

        Ok(Self {
            bbox: parent,
            page_id: page_id.get().into(),
            children_count,
            left_right_split,
            id,
            __phantom: std::marker::PhantomData,
        })
    }
}

impl<const DIMENSION_COUNT: usize, const NODE_SATURATION_POINT: usize, Key, Value> SerializeMinimal
    for crate::dense::structure::Node<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
{
    type ExternalData<'d> = ();

    fn minimally_serialize<'a, 's: 'a, W: std::io::Write>(
        &'a self,
        write_to: &mut W,
        external_data: Self::ExternalData<'s>,
    ) -> std::io::Result<()> {
        let page_id = self.page_id.read().unwrap();
        FastNullablePageId::from(page_id.as_ref().copied()).minimally_serialize(write_to, ())?;

        self.children_count
            .get_maybe_initial(&page_id)
            .minimally_serialize(write_to, ())?;

        match &self.left_right_split.get() {
            Some((l, r)) => {
                (1u8).minimally_serialize(write_to, ())?;

                l.minimally_serialize(write_to, external_data)?;
                r.minimally_serialize(write_to, external_data)?;
            }
            None => {
                (0u8).minimally_serialize(write_to, ())?;
            }
        }

        drop(page_id);

        Ok(())
    }
}
