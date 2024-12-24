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
    pub fn insert(&mut self, k: impl IntoIterator<Item =K>, v: V) -> Option<V> {
        let mut slf = self;

        for key in k {
            let entry = slf.children.entry(key);
            slf = entry.or_insert_with(ModuleTree::new)
        }

        std::mem::replace(&mut slf.value, Some(v))
    }
    pub fn depth_first<'a>(&'a self) -> impl Iterator<Item = &'a V> {
        let mut stack = Vec::new();
        stack.push(self);

        std::iter::from_fn(move || {
            loop {
                let s = stack.pop()?;

                stack.extend(s.children.values());

                if s.value.is_some() {
                    return s.value.as_ref();
                }
            }
        })
    }
}