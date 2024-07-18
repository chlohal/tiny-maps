use std::{collections::BTreeMap, ops::Deref};

use packed_strings::PackedString;

pub type LiteralId = u64;

pub mod packed_strings;
//pub mod building;

pub struct LiteralPool(BTreeMap<String, LiteralId>);

impl LiteralPool {
    pub fn new() -> Self {
        LiteralPool(BTreeMap::new())
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

pub enum Literal {
    KeyVar(LiteralKey, LiteralValue),
    WellKnownKeyVar(WellKnownKeyVar),
    Ref(usize),
}

pub enum LiteralKey {
    WellKnownKey(WellKnownKey),
    Str(PackedString)
}


pub enum WellKnownKeyVar {
    Address,
    MapFeatureType,
}

pub enum LiteralValue {
    BoolYes,
    BoolNo,
    Blank,
    UInt,
    IInt,
    TinyUNumber,
    TinyINumber,
    Date,
    Time,
    ListWithSep,
    TwoUpperLatinAbbrev,
    SplitSemiList,
    SplitCommaList,
}



pub enum WellKnownKey {

}