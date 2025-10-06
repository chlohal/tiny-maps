use std::fs::File;

use minimal_storage::multitype_paged_storage::MultitypePagedStorage;

use crate::{point_range::StoredBinaryTree, sparse::structure::StoredTree};


#[test]
pub fn test() {
    let folder = std::env::current_dir().unwrap().join(".test");
    std::fs::create_dir_all(&folder).unwrap();

    let filename = folder.join("storagetest");
    dbg!(&filename);

    let file = File::options().create(true).append(false).write(true).read(true).open(filename).unwrap();

    let storage = MultitypePagedStorage::open(file);

    let high = 10_000;

    let mut t = StoredTree::<1, 8000, u16, u16>::new_with_storage(0..=high, storage);

    eprintln!("stored tree created...");
    for i in 0..high {
        eprintln!("Inserting...");
        t.insert(i, i);
    }

    eprintln!("all items inserted...");

    t.flush().unwrap();

    for i in 0..high {
        let stored_i = t.get(&i).unwrap();

        assert_eq!(i, stored_i);
    }

    eprintln!("all values are correctly in there!");

    std::fs::remove_dir_all(folder).unwrap();
}