use std::{fs::File, io::Write};

use minimal_storage::{
    paged_storage::{PageId, PagedStorage},
    serialize_min::SerializeMinimal,
};
use osm_tag_compression::compressed_data::{flattened_id, UncompressedOsmData};
use tree::{
    bbox::{BoundingBox, EARTH_BBOX},
    open_tree_dense,
    point_range::DisregardWhenDeserializing,
};

const DATA_SATURATION: usize = 8_000;

fn main() {
    bingbong()
}

fn bingbong() {
    let paged: PagedStorage<
        8,
        tree::dense::structure::Inner<2, DATA_SATURATION, BoundingBox<i32>, UncompressedOsmData>,
    > = PagedStorage::open(File::open(".map/geography/data").unwrap());

    //given is 2767

    let inner = paged
        .get(
            &PageId::new(36),
            (
                &2325.into(),
                &BoundingBox::new(-650390625, 321679687, -648632812, 323437500),
            ),
        )
        .unwrap();

    dbg!(&inner.read());

    let read = inner.read();
    read.minimally_serialize(
        &mut File::create(".map/geography/just-problematic").unwrap(),
        (),
    )
    .unwrap();
}

fn tree() {
    let tree = open_tree_dense::<2, DATA_SATURATION, BoundingBox<i32>, UncompressedOsmData>(
        ".map/geography".into(),
        EARTH_BBOX,
    );

    //REPRO:
    let bermuda = BoundingBox::new(-648897000, 322289000, -646312000, 323858000);
    let (bermuda, _) = bermuda.split_on_axis(&tree::bbox::LongLatSplitDirection::Lat); //verified that right side works fine
    let (bermuda, _) = bermuda.split_on_axis(&tree::bbox::LongLatSplitDirection::Lat); //verified that right side works fine
    let (bermuda, _) = bermuda.split_on_axis(&tree::bbox::LongLatSplitDirection::Lat); //verified that right side works fine
    let (mut bermuda, _) = bermuda.split_on_axis(&tree::bbox::LongLatSplitDirection::Lat); //verified that right side works fine
                                                                                           //both left and right lat splits crash after this point. makes sense because there's 5 divisions by default

    //both left and right `lon` splits crash. ugh that's annoying lol.
    //manually setting
    bermuda.set_y(bermuda.y_end() - 420500);
    bermuda.set_y_end(*bermuda.y());

    bermuda.set_x_end(*bermuda.x());

    dbg!(bermuda);

    dbg!(tree.find_entries_in_box(&bermuda).collect::<Vec<_>>());
}
