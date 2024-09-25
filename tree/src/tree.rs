use std::{
    fs::File,
    io::{Seek, Write},
    path::PathBuf,
};

use btree_vec::BTreeVec;

use crate::{
    structure::{Inner, LongLatTree, Node, Root, TreePagedStorage},
    PAGE_SIZE,
};

use super::{
    point_range::DisregardWhenDeserializing,
    tree_traits::{Dimension, MultidimensionalKey, MultidimensionalParent, MultidimensionalValue},
    NODE_SATURATION_POINT,
};
use minimal_storage::{
    paged_storage::{PageId, PagedStorage},
    serialize_min::{DeserializeFromMinimal, SerializeMinimal},
};

pub type StoredPointTree<const D: usize, K, T> =
    LongLatTree<D, K, DisregardWhenDeserializing<K, T>>;

pub type StoredTree<const D: usize, K, T> = LongLatTree<D, K, T>;

impl<const DIMENSION_COUNT: usize, Key, Value> LongLatTree<DIMENSION_COUNT, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
{
    pub fn new(bbox: Key::Parent, folder: PathBuf) -> Self {
        let mut storage = PagedStorage::open(folder.join("data"));

        let mut structure_file = File::options()
            .write(true)
            .create(true)
            .read(true)
            .open(folder.join("structure"))
            .unwrap();

        let root = if structure_file.metadata().unwrap().len() == 0 {
            let node = Node::new(1, bbox.clone(), &mut storage);

            Root {
                root_bbox: bbox,
                node,
            }
        } else {
            Root::deserialize_minimal(&mut structure_file, &folder).unwrap()
        };

        LongLatTree {
            structure_file,
            root,
            structure_dirty: true,
            storage,
        }
    }

    pub fn flush<'s>(&'s mut self) -> std::io::Result<()> {
        if self.structure_dirty {
            self.structure_file.rewind().unwrap();

            let mut buf = Vec::new();
            self.root.minimally_serialize(&mut buf, ())?;
            self.structure_file.write_all(&buf).unwrap();

            self.structure_dirty = false;
        }

        self.storage.flush();
        Ok(())
    }
    pub fn find_first_item_at_key_exact<'a, 'b>(&'a self, query: &'b Key) -> Option<Value> {
        let (leaf, leaf_bbox) = self.root.search_leaf_for_key(query);

        let delta = query.delta_from_parent(&leaf_bbox);

        let page = self.storage.get(&leaf.page_id, &leaf_bbox).unwrap();

        let item = page.read().children.get(&delta)?.iter().next().cloned();

        item
    }

    pub fn insert(&mut self, k: &Key, item: Value) {
        let (leaf, leaf_bbox, structure_changed) = self
            .root
            .get_key_leaf_splitting_if_needed(k, &mut self.storage);

        let interior_delta_bbox = k.delta_from_parent(&leaf_bbox);
        self.storage
            .get(&leaf, &leaf_bbox)
            .unwrap()
            .write()
            .children
            .push(interior_delta_bbox, item);

        self.structure_dirty |= structure_changed;
    }

    pub fn expand_to_depth(&mut self, depth: usize) {
        self.root.node.expand_to_depth(
            depth,
            &self.root.root_bbox,
            &mut self.storage,
            &Default::default(),
        )
    }

}


impl<const DIMENSION_COUNT: usize, Key, Value> Root<DIMENSION_COUNT, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
{
    //Find the node which would contain the key. If the key is non-pointlike, this may not be a leaf!
    fn search_leaf_for_key<'a>(
        &'a self,
        k: &Key,
    ) -> (&'a Node<DIMENSION_COUNT, Key, Value>, Key::Parent) {
        let mut tree = &self.node;

        let mut bbox = self.root_bbox.to_owned();
        let mut direction =
            <Key::Parent as MultidimensionalParent<DIMENSION_COUNT>>::DimensionEnum::default();

        loop {
            match &tree.left_right_split {
                Some((left, right)) => {
                    let (left_bbox_calculated, right_bbox_calculated) =
                        bbox.split_evenly_on_dimension(&direction);

                    if k.is_contained_in(&left_bbox_calculated) {
                        tree = left;
                        bbox = left_bbox_calculated;
                        direction = direction.next_axis();
                        continue;
                    } else if k.is_contained_in(&right_bbox_calculated) {
                        tree = right;
                        bbox = right_bbox_calculated;
                        direction = direction.next_axis();
                        continue;
                    }
                }
                None => {}
            }

            return (&tree, bbox);
        }
    }

    fn get_key_leaf_splitting_if_needed<'a>(
        &'a mut self,
        k: &Key,
        storage: &mut TreePagedStorage<DIMENSION_COUNT, Key, Value>,
    ) -> (PageId<PAGE_SIZE>, Key::Parent, bool) {
        let mut tree = &mut self.node;
        let mut direction =
            <Key::Parent as MultidimensionalParent<DIMENSION_COUNT>>::DimensionEnum::default();

        let mut structure_changed = false;

        loop {
            match tree.left_right_split {
                Some((ref mut left, ref mut right)) => {
                    if k.is_contained_in(&left.bbox) {
                        tree = left;
                        direction = direction.next_axis();
                        continue;
                    } else if k.is_contained_in(&right.bbox) {
                        tree = right;
                        direction = direction.next_axis();
                        continue;
                    }
                }
                None => {
                    if tree.try_split_left_right(storage, &direction) {
                        structure_changed = true;
                        continue;
                    }
                }
            }

            return (tree.page_id, tree.bbox.to_owned(), structure_changed);
        }
    }
}

impl<const DIMENSION_COUNT: usize, Key, Value> Node<DIMENSION_COUNT, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
{
    pub(crate) fn new(
        id: u64,
        bbox: Key::Parent,
        storage: &mut TreePagedStorage<DIMENSION_COUNT, Key, Value>,
    ) -> Self {
        Self::new_with_children(id, bbox, storage, BTreeVec::new())
    }

    pub(crate) fn new_with_children(
        id: u64,
        bbox: Key::Parent,
        storage: &mut TreePagedStorage<DIMENSION_COUNT, Key, Value>,
        children: BTreeVec<Key::DeltaFromParent, Value>,
    ) -> Self {
        Self {
            bbox: bbox.clone(),
            page_id: storage.new_page(Inner { children }),
            left_right_split: None,
            id,
            __phantom: std::marker::PhantomData,
        }
    }
    pub fn expand_to_depth(
        &mut self,
        depth: usize,
        bbox: &Key::Parent,
        storage: &mut TreePagedStorage<DIMENSION_COUNT, Key, Value>,
        direction: &<Key::Parent as MultidimensionalParent<DIMENSION_COUNT>>::DimensionEnum,
    ) {
        self.try_split_left_right(storage, direction);

        if depth > 1 {
            match self.left_right_split {
                Some((ref mut l, ref mut r)) => {
                    l.expand_to_depth(depth - 1, bbox, storage, direction);
                    r.expand_to_depth(depth - 1, bbox, storage, direction);
                }
                None => {}
            }
        }
    }

    fn try_split_left_right(
        &mut self,
        storage: &mut TreePagedStorage<DIMENSION_COUNT, Key, Value>,
        direction: &<Key::Parent as MultidimensionalParent<DIMENSION_COUNT>>::DimensionEnum,
    ) -> bool {
        if self.left_right_split.is_some() {
            return false;
        }

        let page = storage.get(&self.page_id, &self.bbox).unwrap();
        let inner = page.read();

        if inner.children.len() >= NODE_SATURATION_POINT {
            drop(inner);

            return self.split_left_right_unchecked(storage, direction);
        } else {
            return false;
        }
    }
    fn split_left_right_unchecked(
        &mut self,
        storage: &mut TreePagedStorage<DIMENSION_COUNT, Key, Value>,
        direction: &<Key::Parent as MultidimensionalParent<DIMENSION_COUNT>>::DimensionEnum,
    ) -> bool {
        let (left_bb, right_bb) = self.bbox.split_evenly_on_dimension(direction);

        let mut left_children = BTreeVec::new();
        let mut right_children = BTreeVec::new();

        let page = storage.get(&self.page_id, &self.bbox).unwrap();

        let mut inner = page.write();

        let children = std::mem::take(&mut inner.children);

        for (child_bbox, item) in children.into_iter() {
            let bb_abs = Key::apply_delta_from_parent(&child_bbox, &self.bbox);

            if bb_abs.is_contained_in(&left_bb) {
                left_children.push(bb_abs.delta_from_parent(&left_bb), item);
            } else if bb_abs.is_contained_in(&right_bb) {
                right_children.push(bb_abs.delta_from_parent(&right_bb), item);
            } else {
                inner.children.push(child_bbox, item);
            }
        }

        let (left_id, right_id) = split_id(self.id);

        self.left_right_split = Some((
            Box::new(Node::new_with_children(
                left_id,
                left_bb,
                storage,
                left_children,
            )),
            Box::new(Node::new_with_children(
                right_id,
                right_bb,
                storage,
                right_children,
            )),
        ));

        return true;
    }
}

pub fn make_path(root_path: &PathBuf, id: u64) -> PathBuf {
    let id_hex = format!("{:x}i", id);

    let chunk_size = 4;

    if id_hex.len() < chunk_size {
        root_path.join(id_hex)
    } else if id_hex.len() == chunk_size {
        root_path.join(id_hex).join("i")
    } else {
        root_path
            .join(&id_hex[0..chunk_size])
            .join(&id_hex[chunk_size..])
    }
}

pub(super) fn split_id(id: u64) -> (u64, u64) {
    ((id << 1), (id << 1) | 1)
}
