use std::{
    collections::{BinaryHeap, VecDeque},
    io::{Seek, Write},
    path::PathBuf,
    rc::Rc,
};

use sorted_vec::SortedVec;

use crate::{
    compressor::varint::{from_varint, ToVarint},
    storage::{
        serialize_min::{DeserializeFromMinimal, ReadExtReadOne, SerializeMinimal},
        Storage, StorageReachable,
    },
};

use super::{
    bbox::{BoundingBox, DeltaBoundingBox, DeltaFriendlyU32Offset, LongLatSplitDirection},
    branch_id_creation,
    compare_by::OrderByBBox,
    LongLatTree, RootTreeInfo, StoredTree,
};

impl<T> SerializeMinimal for LongLatTree<T>
where
    T: 'static
        + SerializeMinimal
        + for<'a> DeserializeFromMinimal<ExternalData<'a> = &'a BoundingBox<i32>>
        + Clone,
    for<'s> <T as SerializeMinimal>::ExternalData<'s>: Copy,
{
    type ExternalData<'a> = <T as SerializeMinimal>::ExternalData<'a>;

    fn minimally_serialize<'o, 's: 'o, W: std::io::Write>(
        &'o self,
        write_to: &mut W,
        external_data: Self::ExternalData<'s>,
    ) -> std::io::Result<()> {
        let has_left_right = self.left_right_split.is_some() as u8;
        let has_children = !self.children.is_empty() as u8;

        let mut file = self.root_tree_info.1.try_clone().unwrap();
        let id_based_byte_offset = self.id / 4;
        let offset_in_byte = (self.id % 4) * 2;

        if file.metadata().unwrap().len() < (id_based_byte_offset + 2) {
            file.set_len(id_based_byte_offset + 2)?;
        }
        file.seek(std::io::SeekFrom::Start(id_based_byte_offset))
            .unwrap();
        let mut b = file.read_one().unwrap();

        b |= has_left_right << (offset_in_byte + 1);
        b |= has_children << offset_in_byte;

        file.seek(std::io::SeekFrom::Start(id_based_byte_offset))
            .unwrap();
        file.write_all(&[b]).unwrap();

        drop(file);

        if !self.children.is_empty() {
            self.children.len().write_varint(write_to)?;

            let mut last_bbox = DeltaBoundingBox::<u32>::zero();

            for OrderByBBox(bbox, child) in self.children.iter() {
                let offset = bbox.delta_friendly_offset(&last_bbox);

                offset.minimally_serialize(write_to, ())?;
                child.minimally_serialize(write_to, external_data)?;

                last_bbox = bbox.to_owned();
            }
        }

        Ok(())
    }
}

impl<T> DeserializeFromMinimal for LongLatTree<T>
where
    T: 'static
        + SerializeMinimal
        + for<'a> DeserializeFromMinimal<ExternalData<'a> = &'a BoundingBox<i32>>
        + Clone,
    for<'s> <T as SerializeMinimal>::ExternalData<'s>: Copy,
{
    type ExternalData<'a> = &'a (RootTreeInfo, u64, BoundingBox<i32>);

    fn deserialize_minimal<'a, 'd: 'a, R: std::io::Read>(
        from: &'a mut R,
        external_data: Self::ExternalData<'d>,
    ) -> Result<Self, std::io::Error> {
        let (ref root_tree_info, id, bbox) = external_data;

        let mut file = root_tree_info.1.try_clone().unwrap();
        let id_based_byte_offset = id / 4;
        let offset_in_byte = (id % 4) * 2;

        file.seek(std::io::SeekFrom::Start(id_based_byte_offset))?;
        let header_chunk = file.read_one()?;

        let has_left_right = (header_chunk >> (offset_in_byte + 1)) & 1 == 1;
        let has_children = (header_chunk >> offset_in_byte) & 1 == 1;

        let direction_is_default = id.leading_zeros() % 2 == 1;

        let direction = if direction_is_default {
            LongLatSplitDirection::default()
        } else {
            !LongLatSplitDirection::default()
        };

        let child_len: usize = if has_children { from_varint(from)? } else { 0 };

        let mut children = SortedVec::with_capacity(child_len);

        let mut last_bbox = DeltaFriendlyU32Offset::zero();

        for _ in 0..child_len {
            let delt_delt_bbox = DeltaFriendlyU32Offset::deserialize_minimal(from, ())?;
            let delt_bbox = DeltaBoundingBox::<u32>::from_delta_friendly_offset(&delt_delt_bbox, &last_bbox);
            let abs_bbox = delt_bbox.absolute(&bbox);

            last_bbox = delt_delt_bbox;

            let item = T::deserialize_minimal(from, &abs_bbox)?;

            children.push(OrderByBBox(delt_bbox, item));
        }

        let left_right_split = if has_left_right {
            let (left_path, left_id) = branch_id_creation(&root_tree_info, *id, 0);
            let (right_path, right_id) = branch_id_creation(&root_tree_info, *id, 1);

            let (left_bbox, right_bbox) = bbox.split_on_axis(&direction);

            let left_data = (Rc::clone(&root_tree_info), left_id, left_bbox);
            let right_data = (Rc::clone(&root_tree_info), right_id, right_bbox);

            Some((
                StoredTree::<T>::open(left_path, left_data),
                StoredTree::<T>::open(right_path, right_data),
            ))
        } else {
            None
        };

        Ok(Self {
            root_tree_info: Rc::clone(&external_data.0),
            bbox: bbox.clone(),
            direction,
            children,
            left_right_split,
            id: external_data.1,
        })
    }
}

impl<T> StorageReachable<(RootTreeInfo, u64, BoundingBox<i32>)> for LongLatTree<T>
where
    T: 'static
        + SerializeMinimal
        + for<'a> DeserializeFromMinimal<ExternalData<'a> = &'a BoundingBox<i32>>
        + Clone,
    for<'a> <T as SerializeMinimal>::ExternalData<'a>: Copy,
{
    fn flush_children<'a>(&'a mut self, serialize_data: <Self as SerializeMinimal>::ExternalData<'a>) -> std::io::Result<()> {
        let mut stack = VecDeque::new();

        if let Some((l,r)) = &mut self.left_right_split {
            stack.push_back(l);
            stack.push_back(r);
        }

        while let Some(item) = stack.pop_front() {
            if let Some(result) = item.flush_without_children(serialize_data) {
                result?;
            }

            if let Some((l,r)) = &mut item.ref_mut().left_right_split {
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
