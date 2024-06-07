use std::{collections::HashMap, hash::Hash, ops::Deref};

pub type LiteralId = u64;

pub struct LiteralPool(HashMap<String, LiteralId>);

impl LiteralPool {
    pub fn new() -> Self {
        LiteralPool(HashMap::new())
    }
    
    pub(crate) fn get_id(&mut self, value: impl Deref<Target = str>) -> LiteralId {
        let str = &*value;

        if self.0.contains_key(str) {
            return self.0[str]
        } else {
            let new_id = (self.0.len() + 1) as u64;
            self.0.insert(str.to_owned(), new_id);
            new_id
        }
    }

}