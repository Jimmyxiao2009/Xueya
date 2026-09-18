use crate::ai::embedding::{blob_to_vector, vector_to_blob};
use crate::domain::ProviderConfig;
use crate::error::AppError;
use crate::util::hashing::sha256_text;
use crate::util::time::utc_now;
use sqlx::SqlitePool;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct EmbeddingIndexMetadata {
    pub provider_id: Uuid,
    pub model: String,
    pub dimension: usize,
    pub version: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Stable cache key for one chunk under one provider identity.
///
/// The provider identity includes the id, base URL and embedding model so a
/// user changing any of those does not accidentally reuse vectors from a
/// different embedding space.
pub fn cache_key(
    provider: &ProviderConfig,
    model: &str,
    dimensions: Option<usize>,
    text: &str,
) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let dimensions = dimensions
        .map(|value| value.to_string())
        .unwrap_or_else(|| "auto".to_owned());
    sha256_text(&format!(
        "{}\n{}\n{}\n{}\n{}",
        provider.id, provider.base_url, model, dimensions, normalized
    ))
}

pub async fn get_cached_embedding(
    pool: &SqlitePool,
    cache_key: &str,
) -> Result<Option<Vec<f32>>, AppError> {
    let blob: Option<Vec<u8>> =
        sqlx::query_scalar("SELECT vector_blob FROM embedding_cache WHERE cache_key = ?")
            .bind(cache_key)
            .fetch_optional(pool)
            .await?;
    Ok(blob.and_then(|blob| blob_to_vector(&blob)))
}

pub async fn put_cached_embedding(
    pool: &SqlitePool,
    cache_key: &str,
    provider_id: Uuid,
    model: &str,
    vector: &[f32],
) -> Result<(), AppError> {
    let blob = vector_to_blob(vector);
    sqlx::query(
        "INSERT INTO embedding_cache (cache_key, provider_id, model, dimensions, vector_blob, created_at) \
         VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT(cache_key) DO UPDATE SET \
         dimensions = excluded.dimensions, vector_blob = excluded.vector_blob, created_at = excluded.created_at",
    )
    .bind(cache_key)
    .bind(provider_id)
    .bind(model)
    .bind(vector.len() as i64)
    .bind(blob)
    .bind(utc_now().to_rfc3339())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_index_metadata(
    pool: &SqlitePool,
    provider_id: Uuid,
    model: &str,
) -> Result<Option<EmbeddingIndexMetadata>, AppError> {
    let row = sqlx::query_as::<_, (i64, i64, String)>(
        "SELECT dimension, version, created_at FROM embedding_index_metadata WHERE provider_id = ? AND model = ?",
    )
    .bind(provider_id)
    .bind(model)
    .fetch_optional(pool)
    .await?;
    Ok(row.and_then(|(dimension, version, created_at)| {
        Some(EmbeddingIndexMetadata {
            provider_id,
            model: model.to_owned(),
            dimension: dimension as usize,
            version,
            created_at: chrono::DateTime::parse_from_rfc3339(&created_at)
                .ok()?
                .with_timezone(&chrono::Utc),
        })
    }))
}

pub async fn upsert_index_metadata(
    pool: &SqlitePool,
    provider_id: Uuid,
    model: &str,
    dimension: usize,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO embedding_index_metadata (provider_id, model, dimension, version, created_at) \
         VALUES (?, ?, ?, 1, ?) ON CONFLICT(provider_id, model) DO UPDATE SET \
         dimension = excluded.dimension, version = embedding_index_metadata.version + 1, created_at = excluded.created_at",
    )
    .bind(provider_id)
    .bind(model)
    .bind(dimension as i64)
    .bind(utc_now().to_rfc3339())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn mark_source_index_ready(
    pool: &SqlitePool,
    source_id: Uuid,
    provider_id: Uuid,
    model: &str,
    dimension: usize,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO source_index_state (source_id, provider_id, model, dimension, status, updated_at) \
         VALUES (?, ?, ?, ?, 'ready', ?) ON CONFLICT(source_id, provider_id, model) DO UPDATE SET \
         dimension = excluded.dimension, status = 'ready', updated_at = excluded.updated_at",
    )
    .bind(source_id)
    .bind(provider_id)
    .bind(model)
    .bind(dimension as i64)
    .bind(utc_now().to_rfc3339())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn mark_source_index_needs_reindex(
    pool: &SqlitePool,
    source_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query("UPDATE source_index_state SET status = 'needs_reindex', updated_at = ? WHERE source_id = ?")
        .bind(utc_now().to_rfc3339())
        .bind(source_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// A provider/model/dimension change invalidates every index that is not the
/// current one. The vectors are deleted rather than mixed, so a query can never
/// accidentally compare vectors from two embedding spaces.
pub async fn reconcile_index_metadata(
    pool: &SqlitePool,
    provider_id: Uuid,
    model: &str,
    dimension: usize,
) -> Result<(), AppError> {
    let existing = get_index_metadata(pool, provider_id, model).await?;
    if let Some(metadata) = existing {
        if metadata.dimension == dimension {
            upsert_index_metadata(pool, provider_id, model, dimension).await?;
            return Ok(());
        }
    }

    // Mark all other (and stale same-provider/model) source indexes as needing
    // reindex, then drop vectors for this provider/model before writing the new
    // dimension.
    sqlx::query(
        "UPDATE source_index_state SET status = 'needs_reindex', updated_at = ? \
         WHERE NOT (provider_id = ? AND model = ? AND dimension = ?)",
    )
    .bind(utc_now().to_rfc3339())
    .bind(provider_id)
    .bind(model)
    .bind(dimension as i64)
    .execute(pool)
    .await?;
    sqlx::query("DELETE FROM embeddings WHERE provider_id = ? AND model = ?")
        .bind(provider_id)
        .bind(model)
        .execute(pool)
        .await?;
    upsert_index_metadata(pool, provider_id, model, dimension).await?;
    Ok(())
}

pub async fn source_index_needs_reindex(
    pool: &SqlitePool,
    source_id: Uuid,
) -> Result<bool, AppError> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM source_index_state WHERE source_id = ? AND status = 'needs_reindex'",
    )
    .bind(source_id)
    .fetch_one(pool)
    .await?;
    Ok(count > 0)
}

pub async fn count_embeddings(pool: &SqlitePool) -> Result<i64, AppError> {
    Ok(sqlx::query_scalar("SELECT COUNT(*) FROM embeddings")
        .fetch_one(pool)
        .await?)
}

pub async fn count_cached_embeddings(pool: &SqlitePool) -> Result<i64, AppError> {
    Ok(sqlx::query_scalar("SELECT COUNT(*) FROM embedding_cache")
        .fetch_one(pool)
        .await?)
}

pub async fn count_index_metadata(pool: &SqlitePool) -> Result<i64, AppError> {
    Ok(
        sqlx::query_scalar("SELECT COUNT(*) FROM embedding_index_metadata")
            .fetch_one(pool)
            .await?,
    )
}

pub async fn count_source_index_status(
    pool: &SqlitePool,
    source_id: Uuid,
    status: &str,
) -> Result<i64, AppError> {
    Ok(sqlx::query_scalar(
        "SELECT COUNT(*) FROM source_index_state WHERE source_id = ? AND status = ?",
    )
    .bind(source_id)
    .bind(status)
    .fetch_one(pool)
    .await?)
}
