CREATE TABLE tags(object BIGINT NOT NULL, object_type SMALLINT NOT NULL, key TEXT NOT NULL, value TEXT NOT NULL);
CREATE TABLE parent_relations(child_id BIGINT NOT NULL , child_type SMALLINT NOT NULL , parent_id BIGINT NOT NULL, parent_type SMALLINT NOT NULL, list_index INTEGER NOT NULL, role TEXT);
CREATE TABLE objects(id BIGINT NOT NULL, type SMALLINT NOT NULL);
CREATE TABLE node_longlats(node_id BIGINT NOT NULL, decimicro_long INTEGER NOT NULL, decimicro_lat INTEGER NOT NULL);

CREATE UNIQUE INDEX idx_tag_pk ON tags(object, object_type, key);
CREATE UNIQUE INDEX idx_objects_pk ON objects(id, type);
CREATE UNIQUE INDEX idx_node_longlats_pk ON node_longlats(node_id);

CREATE INDEX idx_tags_by_object ON tags(object, object_type);

CREATE INDEX idx_parent_relations_child ON parent_relations(child_id, child_type);
CREATE INDEX idx_parent_relations_parent ON parent_relations(parent_id, parent_type);