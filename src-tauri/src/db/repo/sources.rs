use crate::domain::{Document, Source, SourceRevision, SourceStatus};
use crate::error::AppError;
use sqlx::SqlitePool;
use uuid::Uuid;

pub async fn insert_source(pool: &SqlitePool, source: &Source) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO sources
            (id, kind, title, uri, original_filename, content_hash, status, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(source.id)
    .bind(source.kind)
    .bind(&source.title)
    .bind(&source.uri)
    .bind(&source.original_filename)
    .bind(&source.content_hash)
    .bind(source.status)
    .bind(source.created_at.to_rfc3339())
    .bind(source.updated_at.to_rfc3339())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn update_source_status(
    pool: &SqlitePool,
    id: Uuid,
    status: SourceStatus,
    content_hash: Option<&str>,
) -> Result<(), AppError> {
    let now = crate::util::time::utc_now().to_rfc3339();
    sqlx::query(
        "UPDATE sources SET status = ?, content_hash = COALESCE(?, content_hash), updated_at = ? WHERE id = ?",
    )
    .bind(status)
    .bind(content_hash)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_sources(pool: &SqlitePool) -> Result<Vec<Source>, AppError> {
    let rows = sqlx::query_as::<_, Source>(
        "SELECT id, kind, title, uri, original_filename, content_hash, status, created_at, updated_at FROM sources ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn get_source(pool: &SqlitePool, id: Uuid) -> Result<Source, AppError> {
    sqlx::query_as::<_, Source>(
        "SELECT id, kind, title, uri, original_filename, content_hash, status, created_at, updated_at FROM sources WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("source {id}")))
}

pub async fn delete_source(pool: &SqlitePool, id: Uuid) -> Result<(), AppError> {
    // Delete derived rows explicitly before the source row. Besides making
    // the authoritative delete independent from SQLite's cascade ordering,
    // this lets the FTS5 delete trigger see each chunk's original text.
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM source_index_state WHERE source_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|error| AppError::Database(format!("delete source index state: {error}")))?;
    sqlx::query("DELETE FROM embeddings WHERE chunk_id IN (SELECT c.id FROM chunks c JOIN documents d ON d.id = c.document_id WHERE d.source_id = ?)")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|error| AppError::Database(format!("delete source embeddings: {error}")))?;
    sqlx::query(
        "DELETE FROM chunks WHERE document_id IN (SELECT id FROM documents WHERE source_id = ?)",
    )
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(|error| AppError::Database(format!("delete source chunks: {error}")))?;
    sqlx::query("DELETE FROM documents WHERE source_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|error| AppError::Database(format!("delete source documents: {error}")))?;
    sqlx::query("DELETE FROM source_revisions WHERE source_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|error| AppError::Database(format!("delete source revisions: {error}")))?;
    sqlx::query("DELETE FROM sources WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|error| AppError::Database(format!("delete source row: {error}")))?;
    tx.commit().await?;
    Ok(())
}

pub async fn find_source_by_hash(
    pool: &SqlitePool,
    content_hash: &str,
) -> Result<Option<Source>, AppError> {
    let row = sqlx::query_as::<_, Source>(
        "SELECT id, kind, title, uri, original_filename, content_hash, status, created_at, updated_at FROM sources WHERE content_hash = ? ORDER BY created_at DESC LIMIT 1",
    )
    .bind(content_hash)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn insert_revision(pool: &SqlitePool, revision: &SourceRevision) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO source_revisions
            (id, source_id, revision_no, content_hash, blob_path, mime_type, size_bytes, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(revision.id)
    .bind(revision.source_id)
    .bind(revision.revision_no)
    .bind(&revision.content_hash)
    .bind(&revision.blob_path)
    .bind(&revision.mime_type)
    .bind(revision.size_bytes)
    .bind(revision.created_at.to_rfc3339())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn next_revision_number(pool: &SqlitePool, source_id: Uuid) -> Result<i64, AppError> {
    let value = sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(MAX(revision_no), 0) + 1 FROM source_revisions WHERE source_id = ?",
    )
    .bind(source_id)
    .fetch_one(pool)
    .await?;
    Ok(value)
}

pub async fn insert_document(pool: &SqlitePool, document: &Document) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO documents
            (id, source_id, revision_id, title, text_content, page_count, metadata_json, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(document.id)
    .bind(document.source_id)
    .bind(document.revision_id)
    .bind(&document.title)
    .bind(&document.text_content)
    .bind(document.page_count)
    .bind(&document.metadata_json)
    .bind(document.created_at.to_rfc3339())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_document(pool: &SqlitePool, id: Uuid) -> Result<Document, AppError> {
    sqlx::query_as::<_, Document>(
        "SELECT id, source_id, revision_id, title, text_content, page_count, metadata_json, created_at FROM documents WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("document {id}")))
}

pub async fn list_documents_by_source(
    pool: &SqlitePool,
    source_id: Uuid,
) -> Result<Vec<Document>, AppError> {
    Ok(sqlx::query_as::<_, Document>(
        "SELECT id, source_id, revision_id, title, text_content, page_count, metadata_json, created_at FROM documents WHERE source_id = ? ORDER BY created_at",
    )
    .bind(source_id)
    .fetch_all(pool)
    .await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{SourceKind, SourceStatus};
    use crate::util::time::utc_now;
    use sqlx::sqlite::SqliteConnectOptions;
    use std::str::FromStr;

    #[tokio::test]
    async fn round_trips_source_enum_and_timestamp() {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")
            .expect("sqlite options")
            .foreign_keys(true);
        let pool = SqlitePool::connect_with(options).await.expect("pool");
        crate::db::migrate::run(&pool).await.expect("migrate");

        let now = utc_now();
        let source = Source {
            id: Uuid::new_v4(),
            kind: SourceKind::File,
            title: "Round trip".into(),
            uri: None,
            original_filename: Some("round-trip.md".into()),
            content_hash: None,
            status: SourceStatus::Importing,
            created_at: now,
            updated_at: now,
        };
        insert_source(&pool, &source).await.expect("insert");
        update_source_status(&pool, source.id, SourceStatus::Ready, Some("abc123"))
            .await
            .expect("update");

        let loaded = get_source(&pool, source.id).await.expect("get");
        assert_eq!(loaded.kind, SourceKind::File);
        assert_eq!(loaded.status, SourceStatus::Ready);
        assert_eq!(loaded.content_hash.as_deref(), Some("abc123"));
        assert_eq!(loaded.title, "Round trip");
    }
}
