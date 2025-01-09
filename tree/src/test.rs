use crate::point_range::StoredBinaryTree;


#[test]
pub fn test() {
    let folder = std::env::current_dir().unwrap().join(".test");
    std::fs::create_dir_all(&folder).unwrap();

    let high = u16::MAX;

    let mut t = StoredBinaryTree::<8000, u16, u16>::new(0..=high, folder.clone());

    for i in 0..high {
        t.insert(i, i);
    }

    t.flush().unwrap();

    for i in 0..high {
        let stored_i = t.get(&i).unwrap();

        assert_eq!(i, stored_i);
    }

    std::fs::remove_dir_all(folder).unwrap();
}