use std::path::PathBuf;

use crate::point_range::StoredBinaryTree;


#[test]
pub fn test() {
    let folder = std::env::current_dir().unwrap().join(".test");
    std::fs::create_dir_all(&folder).unwrap();

    let high = u16::MAX;

    let mut t = StoredBinaryTree::new(0..=high, folder.clone());

    for i in 0..high {
        t.insert(&i, i.into());
    }

    t.flush(()).unwrap();

    let mut found: usize = 0;

    for itm in t.find_entries_in_box(&(0..=high)) {
        found += 1;
        dbg!(itm.0, itm.1.inner());

        assert_eq!(itm.0, *itm.1.inner());
    }

    assert_eq!(found, high as usize);

    for i in 0..high {
        let stored_i = t.find_first_item_at_key_exact(&i).unwrap().inner();

        assert_eq!(i, *stored_i);
    }

    std::fs::remove_dir_all(folder).unwrap();
}