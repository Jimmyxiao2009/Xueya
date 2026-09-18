-- Verruckt Cloud sync journal.
--
-- Authoritative local entities are captured transactionally by triggers,
-- so a crash can never leave a local write out of the outbox. The sync
-- engine builds payloads from the current row at push time; only the
-- entity identity and revision are journaled.

CREATE TABLE IF NOT EXISTS sync_apply_guard (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    active INTEGER NOT NULL DEFAULT 0
);
INSERT OR IGNORE INTO sync_apply_guard (id, active) VALUES (1, 0);

CREATE TABLE IF NOT EXISTS sync_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    account_id TEXT,
    cursor TEXT NOT NULL DEFAULT '',
    bootstrap_done INTEGER NOT NULL DEFAULT 0,
    last_push_at TEXT,
    last_pull_at TEXT,
    last_error TEXT
);
INSERT OR IGNORE INTO sync_state (id, cursor, bootstrap_done) VALUES (1, '', 0);

CREATE TABLE IF NOT EXISTS sync_entity_meta (
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    revision INTEGER NOT NULL DEFAULT 0,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (entity_type, entity_id)
);

CREATE TABLE IF NOT EXISTS sync_outbox (
    change_id TEXT PRIMARY KEY NOT NULL,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    operation TEXT NOT NULL,
    base_revision INTEGER NOT NULL DEFAULT 0,
    revision INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    attempts INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    UNIQUE (entity_type, entity_id)
);
CREATE INDEX IF NOT EXISTS idx_sync_outbox_created ON sync_outbox(created_at);

CREATE TABLE IF NOT EXISTS sync_conflicts (
    id TEXT PRIMARY KEY NOT NULL,
    change_id TEXT,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    server_revision INTEGER NOT NULL,
    client_revision INTEGER NOT NULL,
    server_payload_json TEXT NOT NULL,
    local_payload_json TEXT,
    deleted INTEGER NOT NULL DEFAULT 0,
    resolved INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    resolved_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_sync_conflicts_open ON sync_conflicts(resolved, created_at);

-- Composite local rows (join tables and learner_states) do not have a UUID
-- primary key. The sync protocol uses UUID entity ids, so the engine maps the
-- local id to a deterministic UUID and records the mapping here for tombstones.
CREATE TABLE IF NOT EXISTS sync_entity_ids (
    entity_type TEXT NOT NULL,
    sync_id TEXT NOT NULL,
    local_id TEXT NOT NULL,
    PRIMARY KEY (entity_type, sync_id)
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_sync_entity_ids_local
    ON sync_entity_ids(entity_type, local_id);

-- Source files travel through object storage, not through the delta payload.
-- The revision row keeps the cloud file id so another device can presign a
-- download and then re-index locally.
ALTER TABLE source_revisions ADD COLUMN cloud_file_id TEXT;
ALTER TABLE source_revisions ADD COLUMN cloud_object_key TEXT;

CREATE TABLE IF NOT EXISTS cloud_file_sync (
    source_revision_id TEXT PRIMARY KEY NOT NULL,
    content_hash TEXT NOT NULL,
    file_id TEXT,
    object_key TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    last_error TEXT,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_cloud_file_sync_status ON cloud_file_sync(status);
CREATE INDEX IF NOT EXISTS idx_cloud_file_sync_hash ON cloud_file_sync(content_hash);

CREATE TRIGGER IF NOT EXISTS sync_courses_ai AFTER INSERT ON courses
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'course', lower(hex(NEW.id)), 'upsert',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'course' AND entity_id = lower(hex(NEW.id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'course' AND entity_id = lower(hex(NEW.id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('course', lower(hex(NEW.id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;

CREATE TRIGGER IF NOT EXISTS sync_courses_au AFTER UPDATE ON courses
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'course', lower(hex(NEW.id)), 'upsert',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'course' AND entity_id = lower(hex(NEW.id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'course' AND entity_id = lower(hex(NEW.id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('course', lower(hex(NEW.id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;

CREATE TRIGGER IF NOT EXISTS sync_courses_ad AFTER DELETE ON courses
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'course', lower(hex(OLD.id)), 'delete',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'course' AND entity_id = lower(hex(OLD.id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'course' AND entity_id = lower(hex(OLD.id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('course', lower(hex(OLD.id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;

CREATE TRIGGER IF NOT EXISTS sync_course_nodes_ai AFTER INSERT ON course_nodes
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'course_node', lower(hex(NEW.id)), 'upsert',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'course_node' AND entity_id = lower(hex(NEW.id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'course_node' AND entity_id = lower(hex(NEW.id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('course_node', lower(hex(NEW.id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;

CREATE TRIGGER IF NOT EXISTS sync_course_nodes_au AFTER UPDATE ON course_nodes
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'course_node', lower(hex(NEW.id)), 'upsert',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'course_node' AND entity_id = lower(hex(NEW.id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'course_node' AND entity_id = lower(hex(NEW.id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('course_node', lower(hex(NEW.id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;

CREATE TRIGGER IF NOT EXISTS sync_course_nodes_ad AFTER DELETE ON course_nodes
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'course_node', lower(hex(OLD.id)), 'delete',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'course_node' AND entity_id = lower(hex(OLD.id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'course_node' AND entity_id = lower(hex(OLD.id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('course_node', lower(hex(OLD.id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;

CREATE TRIGGER IF NOT EXISTS sync_course_node_dependencies_ai AFTER INSERT ON course_node_dependencies
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'course_node_dependency', lower(hex(NEW.node_id) || '|' || hex(NEW.depends_on_node_id)), 'upsert',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'course_node_dependency' AND entity_id = lower(hex(NEW.node_id) || '|' || hex(NEW.depends_on_node_id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'course_node_dependency' AND entity_id = lower(hex(NEW.node_id) || '|' || hex(NEW.depends_on_node_id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('course_node_dependency', lower(hex(NEW.node_id) || '|' || hex(NEW.depends_on_node_id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;

CREATE TRIGGER IF NOT EXISTS sync_course_node_dependencies_au AFTER UPDATE ON course_node_dependencies
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'course_node_dependency', lower(hex(NEW.node_id) || '|' || hex(NEW.depends_on_node_id)), 'upsert',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'course_node_dependency' AND entity_id = lower(hex(NEW.node_id) || '|' || hex(NEW.depends_on_node_id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'course_node_dependency' AND entity_id = lower(hex(NEW.node_id) || '|' || hex(NEW.depends_on_node_id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('course_node_dependency', lower(hex(NEW.node_id) || '|' || hex(NEW.depends_on_node_id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;

CREATE TRIGGER IF NOT EXISTS sync_course_node_dependencies_ad AFTER DELETE ON course_node_dependencies
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'course_node_dependency', lower(hex(OLD.node_id) || '|' || hex(OLD.depends_on_node_id)), 'delete',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'course_node_dependency' AND entity_id = lower(hex(OLD.node_id) || '|' || hex(OLD.depends_on_node_id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'course_node_dependency' AND entity_id = lower(hex(OLD.node_id) || '|' || hex(OLD.depends_on_node_id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('course_node_dependency', lower(hex(OLD.node_id) || '|' || hex(OLD.depends_on_node_id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;

CREATE TRIGGER IF NOT EXISTS sync_knowledge_points_ai AFTER INSERT ON knowledge_points
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'knowledge_point', lower(hex(NEW.id)), 'upsert',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'knowledge_point' AND entity_id = lower(hex(NEW.id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'knowledge_point' AND entity_id = lower(hex(NEW.id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('knowledge_point', lower(hex(NEW.id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;

CREATE TRIGGER IF NOT EXISTS sync_knowledge_points_au AFTER UPDATE ON knowledge_points
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'knowledge_point', lower(hex(NEW.id)), 'upsert',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'knowledge_point' AND entity_id = lower(hex(NEW.id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'knowledge_point' AND entity_id = lower(hex(NEW.id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('knowledge_point', lower(hex(NEW.id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;

CREATE TRIGGER IF NOT EXISTS sync_knowledge_points_ad AFTER DELETE ON knowledge_points
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'knowledge_point', lower(hex(OLD.id)), 'delete',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'knowledge_point' AND entity_id = lower(hex(OLD.id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'knowledge_point' AND entity_id = lower(hex(OLD.id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('knowledge_point', lower(hex(OLD.id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;

CREATE TRIGGER IF NOT EXISTS sync_node_knowledge_points_ai AFTER INSERT ON node_knowledge_points
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'node_knowledge_point', lower(hex(NEW.node_id) || '|' || hex(NEW.knowledge_point_id)), 'upsert',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'node_knowledge_point' AND entity_id = lower(hex(NEW.node_id) || '|' || hex(NEW.knowledge_point_id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'node_knowledge_point' AND entity_id = lower(hex(NEW.node_id) || '|' || hex(NEW.knowledge_point_id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('node_knowledge_point', lower(hex(NEW.node_id) || '|' || hex(NEW.knowledge_point_id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;

CREATE TRIGGER IF NOT EXISTS sync_node_knowledge_points_au AFTER UPDATE ON node_knowledge_points
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'node_knowledge_point', lower(hex(NEW.node_id) || '|' || hex(NEW.knowledge_point_id)), 'upsert',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'node_knowledge_point' AND entity_id = lower(hex(NEW.node_id) || '|' || hex(NEW.knowledge_point_id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'node_knowledge_point' AND entity_id = lower(hex(NEW.node_id) || '|' || hex(NEW.knowledge_point_id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('node_knowledge_point', lower(hex(NEW.node_id) || '|' || hex(NEW.knowledge_point_id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;

CREATE TRIGGER IF NOT EXISTS sync_node_knowledge_points_ad AFTER DELETE ON node_knowledge_points
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'node_knowledge_point', lower(hex(OLD.node_id) || '|' || hex(OLD.knowledge_point_id)), 'delete',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'node_knowledge_point' AND entity_id = lower(hex(OLD.node_id) || '|' || hex(OLD.knowledge_point_id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'node_knowledge_point' AND entity_id = lower(hex(OLD.node_id) || '|' || hex(OLD.knowledge_point_id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('node_knowledge_point', lower(hex(OLD.node_id) || '|' || hex(OLD.knowledge_point_id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;

CREATE TRIGGER IF NOT EXISTS sync_lesson_steps_ai AFTER INSERT ON lesson_steps
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'lesson_step', lower(hex(NEW.id)), 'upsert',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'lesson_step' AND entity_id = lower(hex(NEW.id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'lesson_step' AND entity_id = lower(hex(NEW.id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('lesson_step', lower(hex(NEW.id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;

CREATE TRIGGER IF NOT EXISTS sync_lesson_steps_au AFTER UPDATE ON lesson_steps
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'lesson_step', lower(hex(NEW.id)), 'upsert',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'lesson_step' AND entity_id = lower(hex(NEW.id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'lesson_step' AND entity_id = lower(hex(NEW.id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('lesson_step', lower(hex(NEW.id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;

CREATE TRIGGER IF NOT EXISTS sync_lesson_steps_ad AFTER DELETE ON lesson_steps
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'lesson_step', lower(hex(OLD.id)), 'delete',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'lesson_step' AND entity_id = lower(hex(OLD.id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'lesson_step' AND entity_id = lower(hex(OLD.id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('lesson_step', lower(hex(OLD.id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;

CREATE TRIGGER IF NOT EXISTS sync_learner_states_ai AFTER INSERT ON learner_states
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'learner_state', lower(hex(NEW.course_id) || '|' || hex(NEW.knowledge_point_id)), 'upsert',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'learner_state' AND entity_id = lower(hex(NEW.course_id) || '|' || hex(NEW.knowledge_point_id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'learner_state' AND entity_id = lower(hex(NEW.course_id) || '|' || hex(NEW.knowledge_point_id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('learner_state', lower(hex(NEW.course_id) || '|' || hex(NEW.knowledge_point_id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;

CREATE TRIGGER IF NOT EXISTS sync_learner_states_au AFTER UPDATE ON learner_states
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'learner_state', lower(hex(NEW.course_id) || '|' || hex(NEW.knowledge_point_id)), 'upsert',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'learner_state' AND entity_id = lower(hex(NEW.course_id) || '|' || hex(NEW.knowledge_point_id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'learner_state' AND entity_id = lower(hex(NEW.course_id) || '|' || hex(NEW.knowledge_point_id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('learner_state', lower(hex(NEW.course_id) || '|' || hex(NEW.knowledge_point_id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;

CREATE TRIGGER IF NOT EXISTS sync_learner_states_ad AFTER DELETE ON learner_states
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'learner_state', lower(hex(OLD.course_id) || '|' || hex(OLD.knowledge_point_id)), 'delete',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'learner_state' AND entity_id = lower(hex(OLD.course_id) || '|' || hex(OLD.knowledge_point_id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'learner_state' AND entity_id = lower(hex(OLD.course_id) || '|' || hex(OLD.knowledge_point_id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('learner_state', lower(hex(OLD.course_id) || '|' || hex(OLD.knowledge_point_id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;

CREATE TRIGGER IF NOT EXISTS sync_quiz_questions_ai AFTER INSERT ON quiz_questions
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'quiz_question', lower(hex(NEW.id)), 'upsert',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'quiz_question' AND entity_id = lower(hex(NEW.id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'quiz_question' AND entity_id = lower(hex(NEW.id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('quiz_question', lower(hex(NEW.id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;

CREATE TRIGGER IF NOT EXISTS sync_quiz_questions_au AFTER UPDATE ON quiz_questions
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'quiz_question', lower(hex(NEW.id)), 'upsert',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'quiz_question' AND entity_id = lower(hex(NEW.id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'quiz_question' AND entity_id = lower(hex(NEW.id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('quiz_question', lower(hex(NEW.id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;

CREATE TRIGGER IF NOT EXISTS sync_quiz_questions_ad AFTER DELETE ON quiz_questions
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'quiz_question', lower(hex(OLD.id)), 'delete',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'quiz_question' AND entity_id = lower(hex(OLD.id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'quiz_question' AND entity_id = lower(hex(OLD.id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('quiz_question', lower(hex(OLD.id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;

CREATE TRIGGER IF NOT EXISTS sync_quiz_attempts_ai AFTER INSERT ON quiz_attempts
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'quiz_attempt', lower(hex(NEW.id)), 'upsert',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'quiz_attempt' AND entity_id = lower(hex(NEW.id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'quiz_attempt' AND entity_id = lower(hex(NEW.id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('quiz_attempt', lower(hex(NEW.id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;

CREATE TRIGGER IF NOT EXISTS sync_quiz_attempts_au AFTER UPDATE ON quiz_attempts
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'quiz_attempt', lower(hex(NEW.id)), 'upsert',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'quiz_attempt' AND entity_id = lower(hex(NEW.id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'quiz_attempt' AND entity_id = lower(hex(NEW.id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('quiz_attempt', lower(hex(NEW.id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;

CREATE TRIGGER IF NOT EXISTS sync_quiz_attempts_ad AFTER DELETE ON quiz_attempts
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'quiz_attempt', lower(hex(OLD.id)), 'delete',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'quiz_attempt' AND entity_id = lower(hex(OLD.id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'quiz_attempt' AND entity_id = lower(hex(OLD.id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('quiz_attempt', lower(hex(OLD.id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;

CREATE TRIGGER IF NOT EXISTS sync_adaptation_events_ai AFTER INSERT ON adaptation_events
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'adaptation_event', lower(hex(NEW.id)), 'upsert',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'adaptation_event' AND entity_id = lower(hex(NEW.id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'adaptation_event' AND entity_id = lower(hex(NEW.id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('adaptation_event', lower(hex(NEW.id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;

CREATE TRIGGER IF NOT EXISTS sync_adaptation_events_au AFTER UPDATE ON adaptation_events
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'adaptation_event', lower(hex(NEW.id)), 'upsert',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'adaptation_event' AND entity_id = lower(hex(NEW.id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'adaptation_event' AND entity_id = lower(hex(NEW.id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('adaptation_event', lower(hex(NEW.id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;

CREATE TRIGGER IF NOT EXISTS sync_adaptation_events_ad AFTER DELETE ON adaptation_events
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'adaptation_event', lower(hex(OLD.id)), 'delete',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'adaptation_event' AND entity_id = lower(hex(OLD.id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'adaptation_event' AND entity_id = lower(hex(OLD.id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('adaptation_event', lower(hex(OLD.id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;

CREATE TRIGGER IF NOT EXISTS sync_notes_ai AFTER INSERT ON notes
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'note', lower(hex(NEW.id)), 'upsert',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'note' AND entity_id = lower(hex(NEW.id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'note' AND entity_id = lower(hex(NEW.id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('note', lower(hex(NEW.id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;

CREATE TRIGGER IF NOT EXISTS sync_notes_au AFTER UPDATE ON notes
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'note', lower(hex(NEW.id)), 'upsert',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'note' AND entity_id = lower(hex(NEW.id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'note' AND entity_id = lower(hex(NEW.id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('note', lower(hex(NEW.id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;

CREATE TRIGGER IF NOT EXISTS sync_notes_ad AFTER DELETE ON notes
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'note', lower(hex(OLD.id)), 'delete',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'note' AND entity_id = lower(hex(OLD.id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'note' AND entity_id = lower(hex(OLD.id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('note', lower(hex(OLD.id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;

CREATE TRIGGER IF NOT EXISTS sync_knowledge_objects_ai AFTER INSERT ON knowledge_objects
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'knowledge_object', lower(hex(NEW.id)), 'upsert',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'knowledge_object' AND entity_id = lower(hex(NEW.id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'knowledge_object' AND entity_id = lower(hex(NEW.id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('knowledge_object', lower(hex(NEW.id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;

CREATE TRIGGER IF NOT EXISTS sync_knowledge_objects_au AFTER UPDATE ON knowledge_objects
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'knowledge_object', lower(hex(NEW.id)), 'upsert',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'knowledge_object' AND entity_id = lower(hex(NEW.id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'knowledge_object' AND entity_id = lower(hex(NEW.id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('knowledge_object', lower(hex(NEW.id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;

CREATE TRIGGER IF NOT EXISTS sync_knowledge_objects_ad AFTER DELETE ON knowledge_objects
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'knowledge_object', lower(hex(OLD.id)), 'delete',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'knowledge_object' AND entity_id = lower(hex(OLD.id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'knowledge_object' AND entity_id = lower(hex(OLD.id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('knowledge_object', lower(hex(OLD.id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;

CREATE TRIGGER IF NOT EXISTS sync_knowledge_relations_ai AFTER INSERT ON knowledge_relations
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'knowledge_relation', lower(hex(NEW.id)), 'upsert',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'knowledge_relation' AND entity_id = lower(hex(NEW.id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'knowledge_relation' AND entity_id = lower(hex(NEW.id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('knowledge_relation', lower(hex(NEW.id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;

CREATE TRIGGER IF NOT EXISTS sync_knowledge_relations_au AFTER UPDATE ON knowledge_relations
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'knowledge_relation', lower(hex(NEW.id)), 'upsert',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'knowledge_relation' AND entity_id = lower(hex(NEW.id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'knowledge_relation' AND entity_id = lower(hex(NEW.id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('knowledge_relation', lower(hex(NEW.id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;

CREATE TRIGGER IF NOT EXISTS sync_knowledge_relations_ad AFTER DELETE ON knowledge_relations
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'knowledge_relation', lower(hex(OLD.id)), 'delete',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'knowledge_relation' AND entity_id = lower(hex(OLD.id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'knowledge_relation' AND entity_id = lower(hex(OLD.id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('knowledge_relation', lower(hex(OLD.id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;

CREATE TRIGGER IF NOT EXISTS sync_sources_ai AFTER INSERT ON sources
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'source', lower(hex(NEW.id)), 'upsert',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'source' AND entity_id = lower(hex(NEW.id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'source' AND entity_id = lower(hex(NEW.id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('source', lower(hex(NEW.id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;

CREATE TRIGGER IF NOT EXISTS sync_sources_au AFTER UPDATE ON sources
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'source', lower(hex(NEW.id)), 'upsert',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'source' AND entity_id = lower(hex(NEW.id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'source' AND entity_id = lower(hex(NEW.id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('source', lower(hex(NEW.id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;

CREATE TRIGGER IF NOT EXISTS sync_sources_ad AFTER DELETE ON sources
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'source', lower(hex(OLD.id)), 'delete',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'source' AND entity_id = lower(hex(OLD.id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'source' AND entity_id = lower(hex(OLD.id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('source', lower(hex(OLD.id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;

CREATE TRIGGER IF NOT EXISTS sync_source_revisions_ai AFTER INSERT ON source_revisions
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'source_revision', lower(hex(NEW.id)), 'upsert',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'source_revision' AND entity_id = lower(hex(NEW.id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'source_revision' AND entity_id = lower(hex(NEW.id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('source_revision', lower(hex(NEW.id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;

CREATE TRIGGER IF NOT EXISTS sync_source_revisions_au AFTER UPDATE ON source_revisions
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'source_revision', lower(hex(NEW.id)), 'upsert',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'source_revision' AND entity_id = lower(hex(NEW.id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'source_revision' AND entity_id = lower(hex(NEW.id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('source_revision', lower(hex(NEW.id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;

CREATE TRIGGER IF NOT EXISTS sync_source_revisions_ad AFTER DELETE ON source_revisions
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'source_revision', lower(hex(OLD.id)), 'delete',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'source_revision' AND entity_id = lower(hex(OLD.id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'source_revision' AND entity_id = lower(hex(OLD.id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('source_revision', lower(hex(OLD.id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;
