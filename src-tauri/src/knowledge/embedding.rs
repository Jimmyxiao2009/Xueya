use crate::ai::embedding::{blob_to_vector, vector_to_blob};
use crate::error::AppError;
use crate::util::time::utc_now;
use sqlx::SqlitePool;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct EmbeddingRecord {
    pub id: Uuid,
    pub chunk_id: Uuid,
    pub provider_id: Uuid,
    pub model: String,
    pub dimensions: i64,
    pub vector: Vec<f32>,
}

pub async fn upsert_embedding(
    pool: &SqlitePool,
    chunk_id: Uuid,
    provider_id: Uuid,
    model: &str,
    vector: &[f32],
) -> Result<EmbeddingRecord, AppError> {
    if vector.is_empty() {
        return Err(AppError::Embedding(
            "cannot store an empty embedding".into(),
        ));
    }
    if vector.iter().any(|value| !value.is_finite()) {
        return Err(AppError::Embedding(
            "embedding contains non-finite values".into(),
        ));
    }
    let id = Uuid::new_v4();
    let blob = vector_to_blob(vector);
    sqlx::query(
        r#"
        INSERT INTO embeddings (id, chunk_id, provider_id, model, dimensions, vector_blob, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(chunk_id, provider_id, model) DO UPDATE SET
            dimensions = excluded.dimensions,
            vector_blob = excluded.vector_blob,
            created_at = excluded.created_at
        "#,
    )
    .bind(id)
    .bind(chunk_id)
    .bind(provider_id)
    .bind(model)
    .bind(vector.len() as i64)
    .bind(blob)
    .bind(utc_now().to_rfc3339())
    .execute(pool)
    .await?;

    Ok(EmbeddingRecord {
        id,
        chunk_id,
        provider_id,
        model: model.to_owned(),
        dimensions: vector.len() as i64,
        vector: vector.to_vec(),
    })
}

pub async fn delete_embeddings_for_chunk(
    pool: &SqlitePool,
    chunk_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query("DELETE FROM embeddings WHERE chunk_id = ?")
        .bind(chunk_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn load_all_embeddings(pool: &SqlitePool) -> Result<Vec<EmbeddingRecord>, AppError> {
    let rows = sqlx::query_as::<_, EmbeddingRow>(
        "SELECT id, chunk_id, provider_id, model, dimensions, vector_blob FROM embeddings",
    )
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|row| {
            let vector = blob_to_vector(&row.vector_blob)
                .ok_or_else(|| AppError::Embedding("stored embedding blob is malformed".into()))?;
            Ok(EmbeddingRecord {
                id: row.id,
                chunk_id: row.chunk_id,
                provider_id: row.provider_id,
                model: row.model,
                dimensions: row.dimensions,
                vector,
            })
        })
        .collect()
}

#[derive(sqlx::FromRow)]
struct EmbeddingRow {
    id: Uuid,
    chunk_id: Uuid,
    provider_id: Uuid,
    model: String,
    dimensions: i64,
    vector_blob: Vec<u8>,
}
