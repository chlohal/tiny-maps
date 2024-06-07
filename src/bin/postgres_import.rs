use std::{env::args_os, fs::File};

use osmpbfreader::{
    OsmId,
    OsmObj::{Node, Relation, Way},
    Ref,
};

use postgres::{Client, NoTls, Statement, Transaction};

const RELATION_OBJ: i16 = 0;
const WAY_OBJ: i16 = 1;
const NODE_OBJ: i16 = 2;

const COMMIT_EVERY: usize = 100_000;

fn main() -> Result<(), postgres::Error> {
    let filename = args_os()
        .nth(1)
        .expect("Usage: tiny-map-postgres-import [OSMPBF file]");
    let file = File::open(filename).expect("File doesn't exist!");

    let mut reader = osmpbfreader::OsmPbfReader::new(file);

    let mut conn = Client::connect("host=localhost user=postgres", NoTls)?;

    let mut objects_since_last_commit = 0;

    let (
        mut conn,
        mut add_tag_stmt,
        mut add_object_stmt,
        mut add_parent_relation_stmt,
        mut add_node_longlat_stmt,
    ) = reinit_transaction(&mut client)?;

    for obj in reader.par_iter().flatten() {
        if objects_since_last_commit >= COMMIT_EVERY {
            conn.commit()?;
            (
                conn,
                add_tag_stmt,
                add_object_stmt,
                add_parent_relation_stmt,
                add_node_longlat_stmt,
            ) = reinit_transaction(&mut client)?;
            objects_since_last_commit = 0;
        } else {
            objects_since_last_commit += 1;
        }

        let type_id = osm_type_id(&obj.id());

        let obj_id = obj.id().inner_id();

        conn.execute(&add_object_stmt, &[&obj_id, &type_id])?;

        for (k, v) in obj.tags().iter() {
            conn.execute(
                &add_tag_stmt,
                &[&obj_id, &type_id, &k.as_str(), &v.as_str()],
            )?;
        }

        match obj {
            Node(node) => {
                conn.execute(
                    &add_node_longlat_stmt,
                    &[&obj_id, &node.decimicro_lon, &node.decimicro_lat],
                )?;
            }
            Way(way) => {
                for node in way.nodes.iter() {
                    conn.execute(
                        &add_parent_relation_stmt,
                        &[&node.0, &NODE_OBJ, &obj_id, &type_id, &None::<&str>],
                    )?;
                }
            }
            Relation(rel) => {
                for Ref { role, member } in rel.refs.iter() {
                    conn.execute(
                        &add_parent_relation_stmt,
                        &[
                            &member.inner_id(),
                            &osm_type_id(member),
                            &obj_id,
                            &type_id,
                            &Some(role.as_str()),
                        ],
                    )?;
                }
            }
        }
    }

    conn.commit()?;

    Ok(())
}

fn reinit_transaction(
    client: &mut Client,
) -> Result<(Transaction, Statement, Statement, Statement, Statement), postgres::Error> {
    let mut conn = client.transaction()?;

    let add_tag_stmt = conn
        .prepare("INSERT INTO tags (object, object_type, key, value) VALUES ($1, $2, $3, $4);")?;
    let add_object_stmt = conn.prepare("INSERT INTO objects (id, type) VALUES ($1, $2);")?;

    let add_parent_relation_stmt = conn.prepare("INSERT INTO parent_relations (child_id, child_type, parent_id, parent_type, role) VALUES ($1, $2, $3, $4, $5);")?;

    let add_longlat_stmt = conn.prepare(
        "INSERT INTO node_longlats (node_id, decimicro_long, decimicro_lat) VALUES ($1, $2, $3);",
    )?;

    Ok((
        conn,
        add_tag_stmt,
        add_object_stmt,
        add_parent_relation_stmt,
        add_longlat_stmt,
    ))
}

fn osm_type_id(member: &OsmId) -> i16 {
    if member.is_node() {
        return NODE_OBJ;
    }
    if member.is_way() {
        return WAY_OBJ;
    }
    return RELATION_OBJ;
}
