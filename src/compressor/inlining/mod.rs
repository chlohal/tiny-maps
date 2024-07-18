use osmpbfreader::Tags;

pub mod node;

pub struct InlinedTags<'a, InlineObjType> {
    pub inline: InlineObjType,
    pub other: &'a Tags
}



pub trait TagCollection {
    fn has_exactly(&self, tags: &[(&str, &str)]) -> bool;
}


impl TagCollection for Tags {
    fn has_exactly(&self, tags: &[(&str, &str)]) -> bool {
        if tags.len() != self.len() {
            return false;
        }

        for (k,v) in tags {
            if !self.contains(&k, &v) {
                return false;
            }
        }

        return true;
    }
}