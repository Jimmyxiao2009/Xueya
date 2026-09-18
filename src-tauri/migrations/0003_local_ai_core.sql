-- Verruckt Local AI Core schema.
--
-- This migration is additive: existing BYOK providers and imported sources
-- keep working. It adds the production retrieval/embedding pieces:
--   * provider protocol preference
--   * chunk source/route metadata used by route-scoped retrieval
--   * FTS5 lexical index kept in sync by triggers
--   * embedding cache + index metadata so dimensions are never hard-coded
--   * source/session summaries, usage events and capability cache

ALTER TABLE providers ADD COLUMN protocol TEXT NOT NULL DEFAULT 'auto';

ALTER TABLE chunks ADD COLUMN source_id TEXT;
ALTER TABLE chunks ADD COLUMN route_id TEXT;
ALTER TABLE chunks ADD COLUMN text_hash TEXT;
ALTER TABLE chunks ADD COLUMN metadata_json TEXT NOT NULL DEFAULT '{}';
ALTER TABLE chunks ADD COLUMN embedding_status TEXT NOT NULL DEFAULT 'pending';

UPDATE chunks
SET source_id = (SELECT d.source_id FROM documents d WHERE d.id = chunks.document_id)
WHERE source_id IS NULL;

UPDATE chunks
SET text_hash = content_hash
WHERE text_hash IS NULL OR text_hash = '';

CREATE INDEX IF NOT EXISTS idx_chunks_source ON chunks(source_id);
CREATE INDEX IF NOT EXISTS idx_chunks_route ON chunks(route_id);
CREATE INDEX IF NOT EXISTS idx_chunks_embedding_status ON chunks(embedding_status);

-- Standalone FTS5 table. The triggers are the only writer, so stale rows are
-- impossible after insert/update/delete/reindex.
CREATE VIRTUAL TABLE IF NOT EXISTS chunk_fts USING fts5(
    chunk_id UNINDEXED,
    text,
    heading,
    tokenize = 'unicode61'
);

CREATE TRIGGER IF NOT EXISTS chunks_fts_ai AFTER INSERT ON chunks BEGIN
    INSERT INTO chunk_fts(rowid, chunk_id, text, heading)
    VALUES (new.rowid, new.id, new.text, new.heading);
END;

CREATE TRIGGER IF NOT EXISTS chunks_fts_ad AFTER DELETE ON chunks BEGIN
    INSERT INTO chunk_fts(chunk_fts, rowid, chunk_id, text, heading)
    VALUES ('delete', old.rowid, old.id, old.text, old.heading);
END;

CREATE TRIGGER IF NOT EXISTS chunks_fts_au AFTER UPDATE ON chunks BEGIN
    INSERT INTO chunk_fts(chunk_fts, rowid, chunk_id, text, heading)
    VALUES ('delete', old.rowid, old.id, old.text, old.heading);
    INSERT INTO chunk_fts(rowid, chunk_id, text, heading)
    VALUES (new.rowid, new.id, new.text, new.heading);
END;

INSERT INTO chunk_fts(rowid, chunk_id, text, heading)
SELECT c.rowid, c.id, c.text, c.heading
FROM chunks c
WHERE NOT EXISTS (SELECT 1 FROM chunk_fts f WHERE f.chunk_id = c.id);

CREATE TABLE IF NOT EXISTS embedding_index_metadata (
    provider_id TEXT NOT NULL,
    model TEXT NOT NULL,
    dimension INTEGER NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    PRIMARY KEY (provider_id, model)
);

CREATE TABLE IF NOT EXISTS source_index_state (
    source_id TEXT NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
    provider_id TEXT NOT NULL,
    model TEXT NOT NULL,
    dimension INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'ready',
    updated_at TEXT NOT NULL,
    PRIMARY KEY (source_id, provider_id, model)
);

CREATE INDEX IF NOT EXISTS idx_source_index_status ON source_index_state(status);

CREATE TABLE IF NOT EXISTS embedding_cache (
    cache_key TEXT PRIMARY KEY NOT NULL,
    provider_id TEXT NOT NULL,
    model TEXT NOT NULL,
    dimensions INTEGER NOT NULL,
    vector_blob BLOB NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_embedding_cache_provider ON embedding_cache(provider_id, model);

CREATE TABLE IF NOT EXISTS source_summaries (
    source_id TEXT PRIMARY KEY NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
    content_hash TEXT NOT NULL,
    summary TEXT NOT NULL,
    key_points_json TEXT NOT NULL DEFAULT '[]',
    model TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS session_summaries (
    session_id TEXT PRIMARY KEY NOT NULL REFERENCES learning_sessions(id) ON DELETE CASCADE,
    message_count INTEGER NOT NULL DEFAULT 0,
    summary TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS usage_events (
    id TEXT PRIMARY KEY NOT NULL,
    operation TEXT NOT NULL,
    provider_id TEXT,
    model TEXT,
    prompt_version TEXT,
    input_tokens INTEGER,
    output_tokens INTEGER,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_usage_created ON usage_events(created_at);

CREATE TABLE IF NOT EXISTS provider_capability_cache (
    provider_id TEXT PRIMARY KEY NOT NULL,
    capabilities_json TEXT NOT NULL,
    protocol TEXT NOT NULL,
    probed_at TEXT NOT NULL
);
