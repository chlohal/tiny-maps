use std::{collections::BTreeMap, sync::Arc};

use postgres::{
    fallible_iterator::FallibleIterator,
    types::{BorrowToSql, ToSql},
    Error, GenericClient,
};

use crate::tree::bbox::BoundingBox;

pub const RELATION_OBJ: i16 = 0;
pub const WAY_OBJ: i16 = 1;
pub const NODE_OBJ: i16 = 2;

pub struct OsmObject {
    id: i64,
    osm_type: i16,
    lazy_tags: Option<BTreeMap<String, String>>,
    bbox: Option<BoundingBox<i32>>,
}

pub fn osm_objects<'a>(
    conn: &'a mut impl GenericClient,
) -> Result<impl FallibleIterator<Item = OsmObject, Error = Error> + 'a, Error> {
    Ok(conn
        .query_raw::<_, &dyn ToSql, _>("SELECT type, id FROM objects", [])?
        .map(|row| Ok(OsmObject::new(row.get(1), row.get(0)))))
}

pub fn root_osm_objects<'a>(
    conn: &'a mut impl GenericClient,
) -> Result<impl FallibleIterator<Item = OsmObject, Error = Error> + 'a, Error> {
    Ok(conn
        .query_raw::<_, &dyn ToSql, _>(
            "SELECT id, type FROM objects LEFT JOIN parent_relations ON child_id = id AND child_type = type GROUP BY id, type HAVING count(parent_id) = 0;",
            [])?
        .map(|row| Ok(OsmObject::new(row.get(0), row.get(1)))))
}

impl OsmObject {
    fn new(id: i64, osm_type: i16) -> Self {
        OsmObject {
            id,
            osm_type,
            lazy_tags: None,
            bbox: None,
        }
    }
    pub fn tags<'a>(
        &'a mut self,
        conn: &mut impl GenericClient,
    ) -> Result<&'a BTreeMap<String, String>, Error> {
        if let Some(ref tags) = self.lazy_tags {
            return Ok(tags);
        }

        let tags = conn
            .query_raw::<_, &dyn ToSql, _>(
                "SELECT key, value FROM tags WHERE object = $1 AND object_type = $2",
                [&self.id, &NODE_OBJ as &dyn ToSql],
            )?
            .map(|row| Ok((row.get(0), row.get(1))))
            .collect()?;

        self.lazy_tags = Some(tags);
        Ok(self.lazy_tags.as_ref().unwrap())
    }
    pub fn load_bbox<'a>(
        &'a mut self,
        conn: &mut impl GenericClient,
    ) -> Result<BoundingBox<i32>, Error> {
        if let Some(bbox) = self.bbox {
            return Ok(bbox);
        }

        if self.osm_type == NODE_OBJ {
            let row = conn.query_one("SELECT decimicro_long, decimicro_lat FROM node_longlats WHERE node_id = $1", &[&self.id])?;

            return Ok(BoundingBox::from_point(row.get::<_, i32>(0), row.get::<_, i32>(1)));
        }

        let bbox: BoundingBox<i32> = conn
            .query_raw::<_, &dyn ToSql, _>(
                "WITH RECURSIVE children(id, type, x, y) AS (

                    SELECT CAST($1 AS BIGINT), CAST($2 AS SMALLINT), CAST(NULL AS INTEGER), CAST(NULL AS INTEGER)
                    
                    UNION ALL
                    
                    SELECT child_id AS id, child_type AS type, decimicro_long AS x, decimicro_lat AS y 
                    FROM parent_relations
                        INNER JOIN children 
                        ON children.id = parent_relations.parent_id AND
                            children.type = parent_relations.parent_type
                        LEFT JOIN node_longlats 
                        ON node_longlats.node_id = child_id

                )
                SELECT id, x, y FROM children WHERE x IS NOT NULL AND y IS NOT NULL;
                ",
                [&self.id, &NODE_OBJ as &dyn ToSql],
            )?
            .map(|row| Ok((
                row.get(1), row.get(2)
            ))).collect()?;

        self.bbox = Some(bbox);
        Ok(bbox)
    }
    pub fn parents<'a>(
        &mut self,
        conn: &'a mut impl GenericClient,
    ) -> Result<impl FallibleIterator<Item = OsmObject, Error = Error> + 'a, Error> {
        osm_database_object_parents(conn, self.id, self.osm_type)
    }
    pub fn children<'a>(
        &mut self,
        conn: &'a mut impl GenericClient,
    ) -> Result<impl FallibleIterator<Item = OsmObject, Error = Error> + 'a, Error> {
        osm_database_object_children(conn, self.id, self.osm_type)
    }

    pub fn osm_type(&self) -> i16 {
        return self.osm_type;
    }
    pub fn osm_id(&self) -> i64 {
        return self.id;
    }
}

fn osm_database_object_children<'a>(
    client: &'a mut impl GenericClient,
    parent_id: i64,
    parent_type: i16,
) -> Result<impl FallibleIterator<Item = OsmObject, Error = Error> + 'a, Error> {
    Ok(client
        .query_raw::<_, &dyn ToSql, _>(
            "SELECT child_type, child_id FROM parent_relations WHERE parent_id = $1 AND parent_type = $2",
            [&parent_id, &parent_type as &dyn ToSql],
        )?
        .map(|row| Ok(OsmObject::new(row.get(1), row.get(0)))))
}

fn osm_database_object_parents<'a>(
    client: &'a mut impl GenericClient,
    child_id: i64,
    child_type: i16,
) -> Result<impl FallibleIterator<Item = OsmObject, Error = Error> + 'a, Error> {
    Ok(client
        .query_raw::<_, &dyn ToSql, _>(
            "SELECT parent_type, parent_id FROM parent_relations WHERE child_id = $1 AND child_type = $2",
            [&child_id, &child_type as &dyn ToSql],
        )?
        .map(|row| Ok(OsmObject::new(row.get(1), row.get(0)))))
}
