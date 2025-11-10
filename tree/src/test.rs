use std::fs::File;

use minimal_storage::{multitype_paged_storage::{MultitypePagedStorage, StoragePage, StoreByPage}, paged_storage::PageId};

use crate::{point_range::StoredBinaryTree, sparse::{open_file, structure::{Root, StoredTree}, SparseKey, SparseValue}, tree_traits::{MultidimensionalKey, MultidimensionalParent}};

fn open_test_tree<const D: usize, const SATURATION: usize, K: SparseKey<D>, V: SparseValue>(testname: &'static str, parent: K::Parent) -> StoredTree<D, SATURATION, K, V, impl StoragePage<Root<D, SATURATION, K, V>>, impl StoreByPage<crate::sparse::structure::Inner<D, SATURATION, K, V>, PageId = PageId<{ crate::PAGE_SIZE }>>> {
    let folder = std::env::current_dir().unwrap().join(".test");
    std::fs::create_dir_all(&folder).unwrap();

    let filename = folder.join(testname);
    //remove the file if it exists to ensure a clean test; otherwise, ignore it
    let _ = std::fs::remove_file(&filename);


    open_file(parent, filename)
}

macro_rules! funcname {
    () => {
        //from stdext::macros v0.3.1
        {
                fn f() {}
            fn type_name_of<T>(_: T) -> &'static str {
                std::any::type_name::<T>()
            }
            let name = type_name_of(f);
            // `3` is the length of the `::f`.
            &name[..name.len() - 3]
        }
    };
}

#[test]
pub fn basic_tree_test() {
    let high = 10_000;
    let mut t = open_test_tree::<1, 8000, _, _>(funcname!(), 0..=high);

    eprintln!("stored tree created...");
    for i in 0..high {
        eprintln!("Inserting...");
        t.insert(i, i);
    }

    eprintln!("all items inserted...");

    t.flush().unwrap();

    for i in 0..high {
        let stored_i = t.get_owned(&i).unwrap();

        assert_eq!(i, stored_i);
    }

    eprintln!("all values are correctly in there!");
}

#[test]
pub fn overlapping_keys() {
    const NUMBER_INSERT: usize = 2000;
    const RANGE_END_VALUES: usize = 5;
    const SATURATION: usize = 8000;

    let t = open_test_tree::<1, SATURATION, usize, usize>(funcname!(), 0usize..=10usize);

    for value in 0..RANGE_END_VALUES {
        for v in 0..NUMBER_INSERT {
            t.insert(5, value);
        }
        eprintln!("Finished inserting value {value}");
    }

    eprintln!("inserted successfully...");

    //at this point, there should be NUMBER_INSERT*5 items in the `'5' key. 
    // This will most trigger a multi-leaf situation. Let's check that 
    // reading still works in that case.

    let mut num_values_encountered = [0; RANGE_END_VALUES];

    for (k,v) in t.find_entries_in_box(&MultidimensionalParent::UNIVERSE) {
        assert_eq!(k, 5);

        //bound check but more legible
        assert!(v < RANGE_END_VALUES);

        num_values_encountered[v] += 1;
    }

    dbg!(num_values_encountered);

    //check that all the values read are equal
    for (value, num_encountered) in num_values_encountered.iter().enumerate() {
        assert_eq!(*num_encountered, NUMBER_INSERT);
    }
}