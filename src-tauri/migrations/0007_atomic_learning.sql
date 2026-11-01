CREATE TABLE adaptation_keys (
    identity TEXT PRIMARY KEY,
    node_id BLOB NOT NULL REFERENCES course_nodes(id) ON DELETE CASCADE
);
CREATE TABLE learning_outcomes (
    operation_key TEXT PRIMARY KEY,
    payload_json TEXT NOT NULL
);
