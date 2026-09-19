-- Fix FTS5 delete/update maintenance for source cascades.
-- The original standalone-table trigger used the external-content delete
-- command, which can return SQLITE_ERROR when a source cascade removes rows.
DROP TRIGGER IF EXISTS chunks_fts_ad;
DROP TRIGGER IF EXISTS chunks_fts_au;

CREATE TRIGGER IF NOT EXISTS chunks_fts_ad AFTER DELETE ON chunks BEGIN
    DELETE FROM chunk_fts WHERE rowid = old.rowid;
END;

CREATE TRIGGER IF NOT EXISTS chunks_fts_au AFTER UPDATE ON chunks BEGIN
    DELETE FROM chunk_fts WHERE rowid = old.rowid;
    INSERT INTO chunk_fts(rowid, chunk_id, text, heading)
    VALUES (new.rowid, new.id, new.text, new.heading);
END;
