use osm_value_atom::LiteralValue;
use osmpbfreader::Tags;

use osm_structures::{literal::{Literal, LiteralKey}};

pub mod node;
pub mod way;
pub mod relation;


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

static NON_STORED_TAGS: [&'static str; 30] = [

    //tags which are only used for lifecycle/editors
    "source",
    "note",
    "note:ja",
    "note:en",
    "note:city",
    "note:post_town",
    "fixme",
    "comment",

    //Discardable tags
    "KSJ2:curve_id",
    "KSJ2:lat",
    "KSJ2:long",
    "created_by",
    "geobase:datasetName",
    "geobase:uuid",
    "gnis:import_uuid",
    "lat",
    "latitude",
    "lon",
    "longitude",
    "openGeoDB:auto_update",
    "openGeoDB:layer",
    "openGeoDB:version",
    "import_uuid",
    "odbl",
    "odbl:note",
    "sub_sea:type",
    "tiger:separated",
    "tiger:source",
    "tiger:tlid",
    "tiger:upload_uuid",
];

pub(self) fn remove_non_stored_tags(tags: &mut Tags) {
    for tag in NON_STORED_TAGS.iter() {
        tags.remove(*tag);
    }
}