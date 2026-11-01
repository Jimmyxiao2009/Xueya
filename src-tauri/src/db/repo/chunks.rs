use crate::domain::Chunk;
use crate::error::AppError;
use sqlx::Connection;
use sqlx::SqlitePool;
use uuid::Uuid;

pub async fn insert_chunks(pool: &SqlitePool, chunks: &[Chunk]) -> Result<(), AppError> {
    let mut conn = pool.acquire().await?;
    insert_chunks_tx(&mut conn, chunks).await
}

pub async fn insert_chunks_tx(
    conn: &mut sqlx::SqliteConnection,
    chunks: &[Chunk],
) -> Result<(), AppError> {
    let mut transaction = conn.begin().await?;
    for chunk in chunks {
        sqlx::query(
            r#"
            INSERT INTO chunks
                (id, document_id, ordinal, text, char_start, char_end, page_number, heading, token_estimate, content_hash, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(chunk.id)
        .bind(chunk.document_id)
        .bind(chunk.ordinal)
        .bind(&chunk.text)
        .bind(chunk.char_start)
        .bind(chunk.char_end)
        .bind(chunk.page_number)
        .bind(&chunk.heading)
        .bind(chunk.token_estimate)
        .bind(&chunk.content_hash)
        .bind(chunk.created_at.to_rfc3339())
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;
    Ok(())
}

pub async fn list_chunks_by_document(
    pool: &SqlitePool,
    document_id: Uuid,
) -> Result<Vec<Chunk>, AppError> {
    let rows = sqlx::query_as::<_, Chunk>(
        "SELECT id, document_id, ordinal, text, char_start, char_end, page_number, heading, token_estimate, content_hash, created_at FROM chunks WHERE document_id = ? ORDER BY ordinal",
    )
    .bind(document_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn get_chunks_by_ids(pool: &SqlitePool, ids: &[Uuid]) -> Result<Vec<Chunk>, AppError> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = std::iter::repeat_n("?", ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT id, document_id, ordinal, text, char_start, char_end, page_number, heading, token_estimate, content_hash, created_at FROM chunks WHERE id IN ({placeholders})"
    );
    let mut query = sqlx::query_as::<_, Chunk>(&sql);
    for id in ids {
        query = query.bind(id);
    }
    Ok(query.fetch_all(pool).await?)
}

pub async fn get_chunk(pool: &SqlitePool, id: Uuid) -> Result<Chunk, AppError> {
    sqlx::query_as::<_, Chunk>(
        "SELECT id, document_id, ordinal, text, char_start, char_end, page_number, heading, token_estimate, content_hash, created_at FROM chunks WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("chunk {id}")))
}

pub async fn all_chunk_ids(pool: &SqlitePool) -> Result<Vec<Uuid>, AppError> {
    let ids = sqlx::query_scalar::<_, Uuid>("SELECT id FROM chunks")
        .fetch_all(pool)
        .await?;
    Ok(ids)
}
