use std::{cmp::Ordering, ops::Deref};



pub struct CompareBy<T> (pub T, pub fn(&T, &T) -> Ordering);

impl<T> CompareBy<T> {
    pub fn with_cmp(item: T, arg: fn(&T, &T) -> Ordering) -> Self {
        Self(item, arg)
    }
}

impl<T> PartialEq for CompareBy<T> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(&other).is_eq()
    }
}
impl<T> Eq for CompareBy<T> {}


impl<T> PartialOrd for CompareBy<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for CompareBy<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.1(&self.0, &other.0)
    }
}