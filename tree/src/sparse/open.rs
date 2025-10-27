use std::{marker::PhantomData, path::PathBuf};

use minimal_storage::{
    multitype_paged_storage::{MultitypePagedStorage, SingleTypeView, StoragePage, StoreByPage},
    paged_storage::{Page, PageId},
};

use crate::{
    sparse::{
        structure::{Inner, Node, Root, StoredTree},
        SparseKey, SparseValue,
    },
    PAGE_SIZE,
};

pub fn open_file<
    const DIMENSION_COUNT: usize,
    const NODE_SATURATION_POINT: usize,
    Key: SparseKey<DIMENSION_COUNT>,
    Value: SparseValue,
>(
    bbox: Key::Parent,
    storage_file: PathBuf,
) -> StoredTree<
    DIMENSION_COUNT,
    NODE_SATURATION_POINT,
    Key,
    Value,
    Page<
        PAGE_SIZE,
        Root<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value, PageId<PAGE_SIZE>>,
        std::fs::File,
    >,
    SingleTypeView<PAGE_SIZE, std::fs::File, Inner<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>>,
> {
    let storage_file = std::fs::File::options()
        .create(true)
        .read(true)
        .write(true)
        .open(&storage_file)
        .unwrap();

    let storage = MultitypePagedStorage::open(storage_file);

    open_storage(bbox, &storage, PageId::new(1))
}

pub fn open_storage<
    const DIMENSION_COUNT: usize,
    const NODE_SATURATION_POINT: usize,
    Key: SparseKey<DIMENSION_COUNT>,
    Value: SparseValue,
    ThisStorage: StoreByPage<Inner<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>>
        + StoreByPage<
            Root<
                DIMENSION_COUNT,
                NODE_SATURATION_POINT,
                Key,
                Value,
                <ThisStorage as StoreByPage<
                    Inner<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>,
                >>::PageId,
            >,
            PageId: PartialEq + std::fmt::Debug
        >,
>(
    bbox: Key::Parent,
    storage: &ThisStorage,
    root_page_id: <ThisStorage as StoreByPage<
        Root<
            DIMENSION_COUNT,
            NODE_SATURATION_POINT,
            Key,
            Value,
            <ThisStorage as StoreByPage<
                Inner<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>,
            >>::PageId,
        >,
    >>::PageId,
) -> StoredTree<
    DIMENSION_COUNT,
    NODE_SATURATION_POINT,
    Key,
    Value,
    <ThisStorage as StoreByPage<
        Root<
            DIMENSION_COUNT,
            NODE_SATURATION_POINT,
            Key,
            Value,
            <ThisStorage as StoreByPage<
                Inner<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>,
            >>::PageId,
        >,
    >>::Page,
    <ThisStorage as StoreByPage<Inner<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>>>::SubView,
> {
    let root_page = StoreByPage::<Root<_, _, _, _, _>>::get(storage, &root_page_id, ());

    let root = match root_page {
        Some(r) => r,
        None => {
            let actual_root_page_id = storage.new_page_with(|| Root {
                root_bbox: bbox.clone(),
                node: Node::<
                    { DIMENSION_COUNT },
                    { NODE_SATURATION_POINT },
                    Key,
                    Value,
                    _,
                >::new(bbox, storage),
            });

            if root_page_id != actual_root_page_id {
                panic!("Manually specified root page {root_page_id:?} does not match actual {actual_root_page_id:?}")
            }

            StoreByPage::<Root<_, _, _, _, _>>::get(storage, &actual_root_page_id, ()).unwrap()
        }
    };

    let storage = StoreByPage::<Inner<_, _, _, _>>::sub_view(storage);

    StoredTree {
        root,
        storage,
        _sb: PhantomData,
    }
}
