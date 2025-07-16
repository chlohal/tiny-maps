use std::{
    collections::VecDeque,
    fs::File,
    io::{Seek, Write},
    path::PathBuf,
    sync::{
        atomic::Ordering::{Relaxed, SeqCst},
        Mutex, RwLock, RwLockReadGuard, TryLockError,
    },
    usize,
};

use btree_vec::{BTreeVec, SeparateStateIteratable};

use crate::{
    dense::structure::{Inner, Node, Root, StoredTree, TreePagedStorage},
    PAGE_SIZE,
};

use crate::tree_traits::{
    Dimension, MultidimensionalKey, MultidimensionalParent, MultidimensionalValue,
};
use minimal_storage::{
    paged_storage::{Page, PageArcReadLock, PageId, PageReadLock, PageRwLock, PagedStorage},
    serialize_min::{DeserializeFromMinimal, SerializeMinimal},
};

impl<const DIMENSION_COUNT: usize, const NODE_SATURATION_POINT: usize, Key, Value>
    StoredTree<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
{
    pub fn new(bbox: Key::Parent, folder: PathBuf) -> Self {
        let storage_file = std::fs::File::options()
            .create(true)
            .read(true)
            .write(true)
            .open(&folder.join("data"))
            .unwrap();
        let storage = PagedStorage::open(storage_file);

        let mut structure_file = File::options()
            .write(true)
            .create(true)
            .read(true)
            .open(folder.join("structure"))
            .unwrap();

        let root = if structure_file.metadata().unwrap().len() == 0 {
            let node = Node::new(1, bbox.clone());

            Root {
                root_bbox: bbox,
                node,
            }
        } else {
            Root::deserialize_minimal(&mut structure_file, &folder).unwrap()
        };

        StoredTree {
            structure_file,
            root,
            structure_dirty: true.into(),
            storage,
        }
    }

    pub fn flush<'s>(&'s mut self) -> std::io::Result<()> {
        if self.structure_dirty.swap(false, Relaxed) {
            self.structure_file.rewind().unwrap();

            let mut buf = Vec::new();
            self.root.minimally_serialize(&mut buf, ())?;
            self.structure_file.write_all(&buf).unwrap();
        }

        self.storage.flush();
        Ok(())
    }
    pub fn find_items_in_box<'a>(
        &'a self,
        query: &'a Key::Parent,
    ) -> impl Iterator<Item = Value> + 'a {
        self.find_entries_in_box(query).map(|x| x.1)
    }

    pub fn find_entries_in_box<'a>(
        &'a self,
        query: &'a Key::Parent,
    ) -> impl Iterator<Item = (Key, Value)> + 'a {
        self.root
            .search_all_nodes_touching_area(query, usize::MAX)
            .flat_map(move |(node, bbox)| {
                let page_id = node.page_id.read().unwrap();
                let page_read = Page::read_arc(
                    &self
                        .storage
                        .get(
                            page_id.as_ref()?,
                            (&*page_id.as_ref()?, &node.children_count, &bbox),
                        )
                        .unwrap(),
                );

                debug_assert_eq!(
                    page_read.children.len(),
                    node.children_count.get(&page_read)
                );

                drop(page_id);

                let mut iter_state = page_read.children.begin_iteration();

                Some(
                    std::iter::from_fn(move || loop {
                        let (k, v) = page_read.children.stateless_next(&mut iter_state)?;
                        if Key::delta_from_parent_would_be_contained(&k, &bbox, &query) {
                            let k = Key::apply_delta_from_parent(&k, &bbox);
                            return Some((k, v.to_owned()));
                        }
                    })
                    .flat_map(|(k, v)| v.into_iter().map(move |v| (k, v))),
                )
            })
            .flatten()
    }
    pub fn get<'a, 'b>(&'a self, query: &'b Key) -> Option<Value> {
        let (leaf, leaf_bbox) = self.root.search_leaf_for_key(query);

        let delta = query.delta_from_parent(&leaf_bbox);

        let page_id = leaf.page_id.read().unwrap();
        let page = self
            .storage
            .get(
                page_id.as_ref()?,
                (&*page_id.as_ref()?, &leaf.children_count, &leaf_bbox),
            )
            .unwrap();
        debug_assert_eq!(
            page.read().children.len(),
            leaf.children_count.get(&page.read())
        );
        drop(page_id);
        let item = page.read().children.get(&delta)?.iter().next().cloned();

        item
    }

    pub fn root_bbox(&self) -> &Key::Parent {
        &self.root.root_bbox
    }

    fn insert_to_existing_page(
        &self,
        key: <Key as MultidimensionalKey<DIMENSION_COUNT>>::DeltaFromParent,
        value: Value,
        leaf: &Node<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>,
        leaf_bbox: <Key as MultidimensionalKey<DIMENSION_COUNT>>::Parent,
    ) -> Result<
        (),
        (
            Value,
            <Key as MultidimensionalKey<DIMENSION_COUNT>>::DeltaFromParent,
        ),
    > {
        let readlock = leaf.page_id.read().unwrap();

        let page_id = match &*readlock {
            Some(p) => p,
            None => return Err((value, key)),
        };

        let page = self
            .storage
            .get(page_id, (page_id, &leaf.children_count, &leaf_bbox))
            .unwrap();

        let mut page_write = page.write();

        leaf.children_count.increment(&mut *page_write);

        page_write.children.push(key, value);

        Ok(())
    }

    fn insert_to_new_page(
        &self,
        key: <Key as MultidimensionalKey<DIMENSION_COUNT>>::DeltaFromParent,
        value: Value,
        leaf: &Node<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>,
    ) {
        let mut page_id = leaf.page_id.write().unwrap();

        debug_assert!(page_id.is_none());

        let mut children = BTreeVec::new();
        children.push(key, value);

        let mut inner = Inner { children };

        leaf.children_count.set(&mut inner, 1);

        page_id.replace(self.storage.new_page(inner));
    }

    pub fn insert(&self, k: &Key, item: Value) {
        let (leaf, leaf_bbox, _structure_changed) =
            self.root.get_key_leaf_splitting_if_needed(k, &self.storage);

        //Sanity check: the key's leaf should include the key.
        debug_assert!(k.is_contained_in(&leaf_bbox));

        let interior_delta_bbox = k.delta_from_parent(&leaf_bbox);

        let Err((item, interior_delta_bbox)) =
            self.insert_to_existing_page(interior_delta_bbox, item, leaf, leaf_bbox)
        else {
            return;
        };

        self.insert_to_new_page(interior_delta_bbox, item, leaf);

        self.structure_dirty.fetch_or(true, Relaxed);
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

impl<const DIMENSION_COUNT: usize, const NODE_SATURATION_POINT: usize, Key, Value>
    StoredTree<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT, Parent = Key>
        + MultidimensionalParent<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
{
    pub fn find_entries_touching_box<'a>(
        &'a self,
        query: &'a Key::Parent,
        depth: usize,
    ) -> impl Iterator<Item = (Key, Value)> + 'a {
        self.root
            .search_all_nodes_touching_area(query, depth)
            .flat_map(move |(node, bbox)| {
                let page_id = node.page_id.read().unwrap();
                let page_read = Page::read_arc(
                    &self
                        .storage
                        .get(
                            page_id.as_ref()?,
                            (&*page_id.as_ref()?, &node.children_count, &bbox),
                        )
                        .unwrap(),
                );
                drop(page_id);

                let mut iter_state = page_read.children.begin_iteration();
                Some(
                    std::iter::from_fn(move || loop {
                        let (k, v) = page_read.children.stateless_next(&mut iter_state)?;
                        let k = Key::apply_delta_from_parent(k, &bbox);
                        if query.overlaps(&k) {
                            return Some((k, v.to_owned()));
                        }
                    })
                    .flat_map(|(k, v)| v.into_iter().map(move |v| (k, v))),
                )
            })
            .flatten()
    }
}

impl<const DIMENSION_COUNT: usize, const NODE_SATURATION_POINT: usize, Key, Value>
    Root<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
{
    fn search_all_nodes_touching_area<'a>(
        &'a self,
        area: &'a Key::Parent,
        max_depth: usize,
    ) -> impl Iterator<
        Item = (
            &'a Node<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>,
            Key::Parent,
        ),
    > + 'a {
        let mut search_stack = VecDeque::from([(
            &self.node,
            self.root_bbox.to_owned(),
            <Key::Parent as MultidimensionalParent<DIMENSION_COUNT>>::DimensionEnum::default(),
            0,
        )]);

        std::iter::from_fn(move || loop {
            let (parent, bbox, direction, mut depth) = search_stack.pop_front()?;

            if depth > max_depth {
                continue;
            }
            depth += 1;

            match &parent.left_right_split.get() {
                Some((left, right)) => {
                    let (left_bbox_calculated, right_bbox_calculated) =
                        bbox.split_evenly_on_dimension(&direction);

                    if left_bbox_calculated.overlaps(&area) {
                        search_stack.push_back((
                            left,
                            left_bbox_calculated,
                            direction.next_axis(),
                            depth,
                        ));
                    }
                    if right_bbox_calculated.overlaps(&area) {
                        search_stack.push_back((
                            right,
                            right_bbox_calculated,
                            direction.next_axis(),
                            depth,
                        ));
                    }
                }
                None => {}
            }
            return Some((parent, bbox));
        })
    }

    fn search_smallest_node_covering_area(
        &self,
        area: &Key::Parent,
    ) -> (
        &Node<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>,
        Key::Parent,
        <Key::Parent as MultidimensionalParent<DIMENSION_COUNT>>::DimensionEnum,
    ) {
        let mut tree = &self.node;
        let mut bbox = self.root_bbox.to_owned();
        let mut direction =
            <Key::Parent as MultidimensionalParent<DIMENSION_COUNT>>::DimensionEnum::default();

        loop {
            match &tree.left_right_split.get() {
                Some((left, right)) => {
                    let (left_bbox_calculated, right_bbox_calculated) =
                        bbox.split_evenly_on_dimension(&direction);

                    if left_bbox_calculated.contains(&area) {
                        tree = left;
                        bbox = left_bbox_calculated;
                        direction = direction.next_axis();
                        continue;
                    } else if right_bbox_calculated.contains(&area) {
                        tree = right;
                        bbox = right_bbox_calculated;
                        direction = direction.next_axis();
                        continue;
                    }
                }
                None => {}
            }

            return (tree, bbox, direction);
        }
    }

    /// Find the node which would contain the key. If the key is non-pointlike (e.g. an area,
    /// whose Parent type is equivalent to itself), this may not be a leaf! However,
    /// if the key is stored anywhere in the tree, then it's guaranteed that it will be
    /// in the returned node.
    fn search_leaf_for_key<'a>(
        &'a self,
        k: &Key,
    ) -> (
        &'a Node<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>,
        Key::Parent,
    ) {
        let mut tree = &self.node;

        let mut bbox = self.root_bbox.to_owned();
        let mut direction =
            <Key::Parent as MultidimensionalParent<DIMENSION_COUNT>>::DimensionEnum::default();

        loop {
            match &tree.left_right_split.get() {
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
        &'a self,
        k: &Key,
        storage: &TreePagedStorage<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>,
    ) -> (
        &Node<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>,
        Key::Parent,
        bool,
    ) {
        let mut tree = &self.node;
        let mut direction =
            <Key::Parent as MultidimensionalParent<DIMENSION_COUNT>>::DimensionEnum::default();

        //The key should be contained in the root bbox: if not, then we have an issue
        debug_assert!(
            k.is_contained_in(&tree.bbox),
            "Key should be inside root bounding box"
        );

        let mut structure_changed = false;

        loop {
            match tree.left_right_split.get() {
                Some((ref left, ref right)) => {
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

            return (tree, tree.bbox.to_owned(), structure_changed);
        }
    }
}

impl<const DIMENSION_COUNT: usize, const NODE_SATURATION_POINT: usize, Key, Value>
    Node<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>
where
    Key: MultidimensionalKey<DIMENSION_COUNT>,
    Value: MultidimensionalValue<Key>,
{
    pub(crate) fn new(id: u64, bbox: Key::Parent) -> Self {
        Self {
            page_id: None.into(),
            children_count: 0.into(),
            bbox,
            left_right_split: Default::default(),
            __phantom: std::marker::PhantomData,
            id,
        }
    }

    pub(crate) fn new_with_children(
        id: u64,
        bbox: Key::Parent,
        storage: &TreePagedStorage<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>,
        children: BTreeVec<Key::DeltaFromParent, Value>,
    ) -> Self {
        if children.is_empty() {
            return Self::new(id, bbox);
        }

        let children_count = children.len().into();
        Self {
            bbox,
            page_id: Some(storage.new_page(Inner { children })).into(),
            children_count,
            left_right_split: Default::default(),
            id,
            __phantom: std::marker::PhantomData,
        }
    }
    pub fn expand_to_depth(
        &mut self,
        depth: usize,
        bbox: &Key::Parent,
        storage: &TreePagedStorage<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>,
        direction: &<Key::Parent as MultidimensionalParent<DIMENSION_COUNT>>::DimensionEnum,
    ) {
        self.try_split_left_right(storage, direction);

        if depth > 1 {
            match self.left_right_split.get_mut() {
                Some((ref mut l, ref mut r)) => {
                    l.expand_to_depth(depth - 1, bbox, storage, direction);
                    r.expand_to_depth(depth - 1, bbox, storage, direction);
                }
                None => {}
            }
        }
    }

    fn try_split_left_right(
        &self,
        storage: &TreePagedStorage<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>,
        direction: &<Key::Parent as MultidimensionalParent<DIMENSION_COUNT>>::DimensionEnum,
    ) -> bool {
        if self.left_right_split.get().is_some() {
            return false;
        }

        //No consistency promises need to be upheld by `len`, since
        //it's just a check to ensure that we don't split before we have to.
        //If this causes any issues, it'd just be slightly more splitting than
        //purely necessary. Only one thread can run
        //`split_left_right_unchecked`'s init routine at a time, so
        //there's no concern of races between children_count and splitting.
        let len = self
            .children_count
            .get_maybe_initial(&*self.page_id.read().unwrap());

        if len > NODE_SATURATION_POINT {
            return self.split_left_right_unchecked(storage, direction);
        } else {
            return false;
        }
    }
    fn split_left_right_unchecked(
        &self,
        storage: &TreePagedStorage<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>,
        direction: &<Key::Parent as MultidimensionalParent<DIMENSION_COUNT>>::DimensionEnum,
    ) -> bool {
        self.left_right_split.get_or_init(|| {
            let (left_bb, right_bb) = self.bbox.split_evenly_on_dimension(direction);
            let (left_id, right_id) = split_id(self.id);

            let page_id_lock = self.page_id.read().unwrap();
            let Some(page_id) = page_id_lock.as_ref().copied() else {
                return (
                    Box::new(Node::new(left_id, left_bb)),
                    Box::new(Node::new(right_id, right_bb)),
                );
            };

            let page = storage
                .get(&page_id, (&page_id, &self.children_count, &self.bbox))
                .unwrap();

            let mut inner = page.write();
            drop(page_id_lock);

            let mut left_children = BTreeVec::new();
            let mut right_children = BTreeVec::new();

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

            let len = inner.children.len();
            self.children_count.set(&mut inner, len);

            debug_assert_eq!(inner.children.len(), self.children_count.get(&inner));

            if inner.children.len() == 0 {
                let page_id_lock = self.page_id.try_write();
                match page_id_lock {
                    Ok(mut page_id_lock) => {
                        //safety: an exclusive lock on the page represented by
                        //the page ID is held by `inner`.
                        //An exclusive lock is kept on the concept of changing the page ID by
                        //this lock.
                        //All code that observes the `page_id` is enforced to
                        //not use it if it is nulled out.
                        //Worst-case scenario is the reading code gets stale values, but
                        //eventual consistency is maintained
                        unsafe {
                            *page_id_lock = None;
                            page.allow_free();
                        }
                        //ensure dropping after work is done
                        drop(page_id_lock);
                    }
                    //if some other thread is updating the page ID at the same time, then
                    //conservatively don't free it.
                    Err(TryLockError::WouldBlock) => {}
                    lock @ Err(_) => {
                        let _will_imediately_error = lock.unwrap();
                    }
                }
            }

            debug_assert_eq!(inner.children.len(), self.children_count.get(&inner));

            //ensuring that the drop of the `inner` lock happens AFTER the children_count is updated
            drop(inner);

            (
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
            )
        });

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
