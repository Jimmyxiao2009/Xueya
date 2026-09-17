use super::vector_index::VectorIndex;
use crate::ai::traits::EmbeddingProvider;
use crate::db::repo::chunks as chunk_repo;
use crate::domain::Chunk;
use crate::error::AppError;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievedChunk {
    pub chunk: Chunk,
    pub score: f32,
}

pub async fn semantic_search(
    pool: &SqlitePool,
    vector_index: Arc<dyn VectorIndex>,
    embedding_provider: Arc<dyn EmbeddingProvider>,
    provider_id: Uuid,
    model: &str,
    query: &str,
    limit: usize,
) -> Result<Vec<RetrievedChunk>, AppError> {
    if query.trim().is_empty() {
        return Err(AppError::InvalidInput("retrieval query is empty".into()));
    }
    let mut vectors = embedding_provider.embed(vec![query.to_owned()]).await?;
    let query_vector = vectors
        .pop()
        .ok_or_else(|| AppError::Embedding("embedding provider returned no vectors".into()))?;
    let hits = vector_index
        .search(&query_vector, provider_id, model, limit)
        .await?;
    if hits.is_empty() {
        return Ok(Vec::new());
    }

    let ids: Vec<Uuid> = hits.iter().map(|hit| hit.chunk_id).collect();
    let chunks = chunk_repo::get_chunks_by_ids(pool, &ids).await?;
    let mut by_id: HashMap<Uuid, Chunk> =
        chunks.into_iter().map(|chunk| (chunk.id, chunk)).collect();
    let mut results = Vec::with_capacity(hits.len());
    for hit in hits {
        if let Some(chunk) = by_id.remove(&hit.chunk_id) {
            results.push(RetrievedChunk {
                chunk,
                score: hit.score,
            });
        }
    }
    results.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(results)
}
