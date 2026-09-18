use super::embedding::{
    delete_embeddings_for_chunk, load_all_embeddings, upsert_embedding, EmbeddingRecord,
};
use crate::ai::embedding::cosine_similarity;
use crate::error::AppError;
use async_trait::async_trait;
use sqlx::SqlitePool;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct VectorSearchResult {
    pub chunk_id: Uuid,
    pub score: f32,
}

#[async_trait]
pub trait VectorIndex: Send + Sync {
    async fn upsert(
        &self,
        chunk_id: Uuid,
        provider_id: Uuid,
        model: &str,
        vector: &[f32],
    ) -> Result<(), AppError>;
    async fn delete(&self, chunk_id: Uuid) -> Result<(), AppError>;
    async fn search(
        &self,
        query: &[f32],
        provider_id: Uuid,
        model: &str,
        limit: usize,
    ) -> Result<Vec<VectorSearchResult>, AppError>;

    /// Search only a pre-filtered set of chunks.
    ///
    /// The default implementation is correct but intentionally conservative;
    /// `SqliteVectorIndex` overrides it so route filters shrink the scan before
    /// cosine similarity is computed.
    async fn search_filtered(
        &self,
        query: &[f32],
        provider_id: Uuid,
        model: &str,
        allowed_chunk_ids: Option<&HashSet<Uuid>>,
        limit: usize,
    ) -> Result<Vec<VectorSearchResult>, AppError> {
        let hits = self
            .search(
                query,
                provider_id,
                model,
                limit.saturating_mul(4).max(limit),
            )
            .await?;
        Ok(hits
            .into_iter()
            .filter(|hit| {
                allowed_chunk_ids
                    .map(|allowed| allowed.contains(&hit.chunk_id))
                    .unwrap_or(true)
            })
            .take(limit)
            .collect())
    }

    async fn rebuild(&self) -> Result<(), AppError>;
}

type EmbeddingCacheKey = (Uuid, Uuid, String);
type EmbeddingCache = Arc<RwLock<HashMap<EmbeddingCacheKey, EmbeddingRecord>>>;

pub struct SqliteVectorIndex {
    pool: SqlitePool,
    cache: EmbeddingCache,
    loaded: AtomicBool,
}

impl SqliteVectorIndex {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            cache: Arc::new(RwLock::new(HashMap::new())),
            loaded: AtomicBool::new(false),
        }
    }

    async fn ensure_loaded(&self) -> Result<(), AppError> {
        if !self.loaded.load(Ordering::Acquire) {
            self.rebuild().await?;
        }
        Ok(())
    }
}

#[async_trait]
impl VectorIndex for SqliteVectorIndex {
    async fn upsert(
        &self,
        chunk_id: Uuid,
        provider_id: Uuid,
        model: &str,
        vector: &[f32],
    ) -> Result<(), AppError> {
        let record = upsert_embedding(&self.pool, chunk_id, provider_id, model, vector).await?;
        self.cache.write().await.insert(
            (record.chunk_id, record.provider_id, record.model.clone()),
            record,
        );
        self.loaded.store(true, Ordering::Release);
        Ok(())
    }

    async fn delete(&self, chunk_id: Uuid) -> Result<(), AppError> {
        delete_embeddings_for_chunk(&self.pool, chunk_id).await?;
        self.cache
            .write()
            .await
            .retain(|(cached_chunk_id, _, _), _| *cached_chunk_id != chunk_id);
        Ok(())
    }

    async fn search(
        &self,
        query: &[f32],
        provider_id: Uuid,
        model: &str,
        limit: usize,
    ) -> Result<Vec<VectorSearchResult>, AppError> {
        if query.is_empty() {
            return Err(AppError::Embedding("query embedding is empty".into()));
        }
        self.ensure_loaded().await?;
        let cache = self.cache.read().await;
        let mut matches = Vec::new();
        for record in cache.values() {
            if record.provider_id != provider_id || record.model != model {
                continue;
            }
            if record.dimensions != query.len() as i64 || record.vector.len() != query.len() {
                return Err(AppError::Embedding(format!(
                    "embedding dimension mismatch: query has {}, indexed {}",
                    query.len(),
                    record.vector.len()
                )));
            }
            if let Some(score) = cosine_similarity(query, &record.vector) {
                matches.push(VectorSearchResult {
                    chunk_id: record.chunk_id,
                    score,
                });
            }
        }
        matches.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        matches.truncate(limit.clamp(1, 100));
        Ok(matches)
    }

    async fn search_filtered(
        &self,
        query: &[f32],
        provider_id: Uuid,
        model: &str,
        allowed_chunk_ids: Option<&HashSet<Uuid>>,
        limit: usize,
    ) -> Result<Vec<VectorSearchResult>, AppError> {
        if query.is_empty() {
            return Err(AppError::Embedding("query embedding is empty".into()));
        }
        self.ensure_loaded().await?;
        let cache = self.cache.read().await;
        let mut matches = Vec::new();
        for record in cache.values() {
            if record.provider_id != provider_id || record.model != model {
                continue;
            }
            if let Some(allowed) = allowed_chunk_ids {
                if !allowed.contains(&record.chunk_id) {
                    continue;
                }
            }
            if record.dimensions != query.len() as i64 || record.vector.len() != query.len() {
                // A stale vector from a previous embedding model/dimension is
                // never mixed with the current query. The source is flagged
                // needs_reindex separately; here we simply exclude it.
                tracing::warn!(
                    chunk_id = %record.chunk_id,
                    expected = query.len(),
                    found = record.vector.len(),
                    "skipping stale embedding with mismatched dimension"
                );
                continue;
            }
            if let Some(score) = cosine_similarity(query, &record.vector) {
                matches.push(VectorSearchResult {
                    chunk_id: record.chunk_id,
                    score,
                });
            }
        }
        matches.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        matches.truncate(limit.clamp(1, 100));
        Ok(matches)
    }

    async fn rebuild(&self) -> Result<(), AppError> {
        let records = load_all_embeddings(&self.pool).await?;
        let mut cache = self.cache.write().await;
        cache.clear();
        for record in records {
            cache.insert(
                (record.chunk_id, record.provider_id, record.model.clone()),
                record,
            );
        }
        self.loaded.store(true, Ordering::Release);
        Ok(())
    }
}
