use osmpbfreader::Tags;

use super::literals::{literal_value::LiteralValue, Literal, LiteralKey};

pub mod node;


#[derive(Clone, Debug)]
pub struct InlinedTags<InlineObjType: Clone> {
    pub inline: InlineObjType,
    pub other: Vec<Literal>,
}

pub trait TagCollection {
    fn drain_to_literal_list(&mut self) -> Vec<Literal>;
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
    fn drain_to_literal_list(&mut self) -> Vec<Literal> {
        let mut list = Vec::new();

        self.retain(|k, v| {
            let k_packed: LiteralKey = LiteralKey::from(k);
            let v_packed = LiteralValue::from(v);

            list.push(Literal::KeyVar(k_packed, v_packed));

            true
        });

        list
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
