use std::collections::BTreeMap;

use osmpbfreader::Tags;

use super::literals::{literal_value::LiteralValue, packed_strings::PackedString};

pub mod node;

pub struct InlinedTags<InlineObjType> {
    pub inline: InlineObjType,
    pub other: BTreeMap<PackedString, LiteralValue>,
}

pub trait TagCollection {
    fn drain_to_literal_map(&mut self) -> BTreeMap<PackedString, LiteralValue>;
    fn has_exactly(&self, tags: &[(&str, &str)]) -> bool;
    fn has_subset(&self, tags: &[(&str, &str)]) -> bool;
}

impl TagCollection for Tags {
    fn has_exactly(&self, tags: &[(&str, &str)]) -> bool {
        if tags.len() != self.len() {
            return false;
        }

        for (k, v) in tags {
            if !self.contains(&k, &v) {
                return false;
            }
        }

        return true;
    }
    fn drain_to_literal_map(&mut self) -> BTreeMap<PackedString, LiteralValue> {
        let mut map = BTreeMap::new();

        self.retain(|k, v| {
            let k_packed = PackedString::from(k);
            let v_packed = LiteralValue::from(v);

            map.insert(k_packed, v_packed);

            true
        });

        map
    }
    fn has_subset(&self, tags: &[(&str, &str)]) -> bool {
        for (k, v) in tags {
            if !self.contains(&k, &v) {
                return false;
            }
        }

        return true;
    }
}
