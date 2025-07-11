use osmpbfreader::Tags;


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

pub fn remove_non_stored_tags(tags: &mut Tags) {
    for tag in NON_STORED_TAGS.iter() {
        tags.remove(*tag);
    }
}