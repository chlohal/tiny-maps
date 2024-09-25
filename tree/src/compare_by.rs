use std::fmt::Debug;

pub struct OrderByFirst<Key, Value>(pub Key, pub Value);

impl<K: PartialOrd,V> PartialOrd for OrderByFirst<K,V> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl<K: Debug, V: Debug> Debug for OrderByFirst<K,V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("").field(&self.0).field(&self.1).finish()
    }
}

impl<K: Eq,V> Eq for OrderByFirst<K,V> {}


impl<K: PartialEq,V> PartialEq for OrderByFirst<K,V> {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl<K: Ord,V> Ord for OrderByFirst<K,V> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}