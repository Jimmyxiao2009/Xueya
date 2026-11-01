-- Socratic learning-state sync triggers.
--
-- Local Core remains authoritative. These triggers journal the session,
-- conversation, attempts and mastery state so another device can resume the
-- same learning thread without the cloud becoming the learning runtime.

CREATE TRIGGER IF NOT EXISTS sync_learning_sessions_ai AFTER INSERT ON learning_sessions
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'learning_session', lower(hex(NEW.id)), 'upsert',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'learning_session' AND entity_id = lower(hex(NEW.id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'learning_session' AND entity_id = lower(hex(NEW.id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('learning_session', lower(hex(NEW.id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;
CREATE TRIGGER IF NOT EXISTS sync_learning_sessions_au AFTER UPDATE ON learning_sessions
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'learning_session', lower(hex(NEW.id)), 'upsert',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'learning_session' AND entity_id = lower(hex(NEW.id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'learning_session' AND entity_id = lower(hex(NEW.id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('learning_session', lower(hex(NEW.id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;
CREATE TRIGGER IF NOT EXISTS sync_learning_sessions_ad AFTER DELETE ON learning_sessions
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'learning_session', lower(hex(OLD.id)), 'delete',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'learning_session' AND entity_id = lower(hex(OLD.id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'learning_session' AND entity_id = lower(hex(OLD.id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('learning_session', lower(hex(OLD.id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;
CREATE TRIGGER IF NOT EXISTS sync_learning_messages_ai AFTER INSERT ON learning_messages
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'learning_message', lower(hex(NEW.id)), 'upsert',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'learning_message' AND entity_id = lower(hex(NEW.id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'learning_message' AND entity_id = lower(hex(NEW.id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('learning_message', lower(hex(NEW.id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;
CREATE TRIGGER IF NOT EXISTS sync_learning_messages_au AFTER UPDATE ON learning_messages
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'learning_message', lower(hex(NEW.id)), 'upsert',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'learning_message' AND entity_id = lower(hex(NEW.id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'learning_message' AND entity_id = lower(hex(NEW.id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('learning_message', lower(hex(NEW.id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;
CREATE TRIGGER IF NOT EXISTS sync_learning_messages_ad AFTER DELETE ON learning_messages
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'learning_message', lower(hex(OLD.id)), 'delete',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'learning_message' AND entity_id = lower(hex(OLD.id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'learning_message' AND entity_id = lower(hex(OLD.id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('learning_message', lower(hex(OLD.id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;
CREATE TRIGGER IF NOT EXISTS sync_learning_attempts_ai AFTER INSERT ON learning_attempts
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'learning_attempt', lower(hex(NEW.id)), 'upsert',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'learning_attempt' AND entity_id = lower(hex(NEW.id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'learning_attempt' AND entity_id = lower(hex(NEW.id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('learning_attempt', lower(hex(NEW.id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;
CREATE TRIGGER IF NOT EXISTS sync_learning_attempts_au AFTER UPDATE ON learning_attempts
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'learning_attempt', lower(hex(NEW.id)), 'upsert',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'learning_attempt' AND entity_id = lower(hex(NEW.id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'learning_attempt' AND entity_id = lower(hex(NEW.id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('learning_attempt', lower(hex(NEW.id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;
CREATE TRIGGER IF NOT EXISTS sync_learning_attempts_ad AFTER DELETE ON learning_attempts
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'learning_attempt', lower(hex(OLD.id)), 'delete',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'learning_attempt' AND entity_id = lower(hex(OLD.id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'learning_attempt' AND entity_id = lower(hex(OLD.id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('learning_attempt', lower(hex(OLD.id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;
CREATE TRIGGER IF NOT EXISTS sync_mastery_states_ai AFTER INSERT ON mastery_states
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'mastery_state', lower(hex(NEW.node_id)), 'upsert',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'mastery_state' AND entity_id = lower(hex(NEW.node_id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'mastery_state' AND entity_id = lower(hex(NEW.node_id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('mastery_state', lower(hex(NEW.node_id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;
CREATE TRIGGER IF NOT EXISTS sync_mastery_states_au AFTER UPDATE ON mastery_states
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'mastery_state', lower(hex(NEW.node_id)), 'upsert',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'mastery_state' AND entity_id = lower(hex(NEW.node_id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'mastery_state' AND entity_id = lower(hex(NEW.node_id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('mastery_state', lower(hex(NEW.node_id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;
CREATE TRIGGER IF NOT EXISTS sync_mastery_states_ad AFTER DELETE ON mastery_states
WHEN NOT EXISTS (SELECT 1 FROM sync_apply_guard WHERE id = 1 AND active = 1)
BEGIN
    INSERT INTO sync_outbox (change_id, entity_type, entity_id, operation, base_revision, revision, created_at)
    VALUES (lower(hex(randomblob(16))), 'mastery_state', lower(hex(OLD.node_id)), 'delete',
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'mastery_state' AND entity_id = lower(hex(OLD.node_id))), 0),
        COALESCE((SELECT revision FROM sync_entity_meta WHERE entity_type = 'mastery_state' AND entity_id = lower(hex(OLD.node_id))), 0) + 1,
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        change_id = excluded.change_id,
        operation = excluded.operation,
        revision = excluded.revision,
        created_at = excluded.created_at;
    INSERT INTO sync_entity_meta (entity_type, entity_id, revision, updated_at)
    VALUES ('mastery_state', lower(hex(OLD.node_id)), 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    ON CONFLICT(entity_type, entity_id) DO UPDATE SET
        revision = revision + 1,
        updated_at = excluded.updated_at;
END;
