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
    Page<PAGE_SIZE, Root<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>, std::fs::File>,
    SingleTypeView<
        PAGE_SIZE,
        std::fs::File,
        Inner<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>,
    >,
> {
    let storage_file = std::fs::File::options()
        .create(true)
        .read(true)
        .write(true)
        .open(&storage_file)
        .unwrap();

    let storage = MultitypePagedStorage::open(storage_file);

    open_storage(bbox, &storage, Some(PageId::new(1)))
}

pub fn open_storage<
    const DIMENSION_COUNT: usize,
    const NODE_SATURATION_POINT: usize,
    Key: SparseKey<DIMENSION_COUNT>,
    Value: SparseValue,
    ThisStorage: StoreByPage<Inner<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>, PageId = PageId<PAGE_SIZE>>
        + StoreByPage<
            Root<
                DIMENSION_COUNT,
                NODE_SATURATION_POINT,
                Key,
                Value,
            >,
            PageId = PageId<PAGE_SIZE>
        >,
>(
    bbox: Key::Parent,
    storage: &ThisStorage,
    root_page_id: Option<PageId<PAGE_SIZE>>,
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
        >,
    >>::Page,
    <ThisStorage as StoreByPage<Inner<DIMENSION_COUNT, NODE_SATURATION_POINT, Key, Value>>>::SubView,
>{
    let root_page = root_page_id
        .and_then(|id| StoreByPage::<Root<_, _, _, _>>::get(storage, &id, ()));

    let (root_page_id, root) = match root_page {
        Some(r) => (root_page_id.unwrap(), r),
        None => {
            let actual_root_page_id = storage.new_page_with(|| Root {
                root_bbox: bbox.clone(),
                node: Node::<{ DIMENSION_COUNT }, { NODE_SATURATION_POINT }, Key, Value>::new(
                    bbox, storage,
                ),
            });

            if root_page_id.is_some_and(|id| id != actual_root_page_id) {
                panic!("Manually specified root page {root_page_id:?} does not match actual {actual_root_page_id:?}")
            }

            (
                actual_root_page_id,
                StoreByPage::<Root<_, _, _, _>>::get(storage, &actual_root_page_id, ()).unwrap(),
            )
        }
    };

    let storage = StoreByPage::<Inner<_, _, _, _>>::sub_view(storage);

    StoredTree {
        root,
        storage,
        root_page_id,
        _sb: PhantomData,
    }
}
