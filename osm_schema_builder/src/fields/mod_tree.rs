use std::{collections::HashMap, hash::Hash};

pub struct ModuleTree<K, V> {
    pub value: Option<V>,
    pub children: HashMap<K, ModuleTree<K, V>>
}

impl<K:Hash+Eq,V> ModuleTree<K,V> {
    pub fn new() -> Self {
        Self {
            value: None,
            children: HashMap::new(),
        }
    }
}