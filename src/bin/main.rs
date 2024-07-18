use std::{collections::HashSet, env::args_os, fs::File};

use offline_tiny_maps::{
    postgres_objects::{osm_objects, root_osm_objects, NODE_OBJ},
    tree::{
        bbox::{BoundingBox, EARTH_BBOX},
        LongLatTree,
    },
};
use postgres::{fallible_iterator::FallibleIterator, Client, NoTls};

fn main() -> Result<(), postgres::Error> {
    let querystring = "host=localhost dbname=chlohal user=postgres password=passford";

    let mut client = Client::connect(&querystring, NoTls)?;

    let mut iterclient = Client::connect(querystring, NoTls)?;

    let objs = osm_objects(&mut client)?;

    let mut result = LongLatTree::new(EARTH_BBOX);

    objs.for_each(|mut obj| {
        let tags = obj.tags(&mut iterclient)?;

        let bbox = obj.load_bbox(&mut iterclient)?;

        result.insert(bbox, obj);

        Ok(())
    })?;

    Ok(())
}
