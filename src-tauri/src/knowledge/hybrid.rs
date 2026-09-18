use super::fts::lexical_search;
use crate::db::repo::{chunks as chunk_repo, courses as course_repo, sources as source_repo};
use crate::domain::Chunk;
use crate::error::AppError;
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetrievalFilter {
    pub route_id: Option<Uuid>,
    pub source_ids: Option<Vec<Uuid>>,
    pub knowledge_point_ids: Option<Vec<Uuid>>,
    #[serde(default = "default_top_k")]
    pub top_k: usize,
}

fn default_top_k() -> usize {
    8
}

impl RetrievalFilter {
    pub fn for_route(route_id: Uuid) -> Self {
        Self {
            route_id: Some(route_id),
            top_k: 8,
            ..Self::default()
        }
    }

    pub fn with_top_k(mut self, top_k: usize) -> Self {
        self.top_k = top_k.clamp(1, 20);
        self
    }
}

#[derive(Debug, Clone)]
pub struct HybridHit {
    pub chunk_id: Uuid,
    pub source_id: Uuid,
    pub source_title: String,
    pub text: String,
    pub vector_rank: Option<usize>,
    pub lexical_rank: Option<usize>,
    pub vector_score: Option<f32>,
    pub lexical_score: Option<f32>,
    pub combined_score: f32,
    pub chunk: Chunk,
}

impl HybridHit {
    pub fn into_retrieved(self) -> crate::knowledge::retrieval::RetrievedChunk {
        crate::knowledge::retrieval::RetrievedChunk {
            chunk: self.chunk,
            score: self.combined_score,
        }
    }
}

pub struct HybridRetriever<'a> {
    state: &'a AppState,
}

impl<'a> HybridRetriever<'a> {
    pub fn new(state: &'a AppState) -> Self {
        Self { state }
    }

    pub async fn retrieve(
        &self,
        query: &str,
        filter: RetrievalFilter,
    ) -> Result<Vec<HybridHit>, AppError> {
        if query.trim().is_empty() {
            return Err(AppError::InvalidInput("retrieval query is empty".into()));
        }
        let top_k = filter.top_k.clamp(1, 20);
        let allowed = self.allowed_chunk_ids(&filter).await?;
        if matches!(allowed, Some(ref ids) if ids.is_empty()) {
            return Ok(Vec::new());
        }

        // Vector search degrades gracefully: a configured chat endpoint with a
        // broken embedding endpoint must not take the whole tutor down.
        let vector_hits = match self.vector_hits(query, &allowed, top_k * 2).await {
            Ok(hits) => hits,
            Err(error) => {
                tracing::warn!(%error, "vector retrieval unavailable; falling back to lexical only");
                Vec::new()
            }
        };
        let lexical_hits = lexical_search(&self.state.pool, query, top_k * 2, allowed.as_ref())
            .await
            .unwrap_or_default();

        let mut vector_rank = HashMap::new();
        let mut vector_score = HashMap::new();
        for (index, hit) in vector_hits.iter().enumerate() {
            vector_rank.insert(hit.chunk_id, index + 1);
            vector_score.insert(hit.chunk_id, hit.score);
        }
        let mut lexical_rank = HashMap::new();
        let mut lexical_score = HashMap::new();
        for (index, hit) in lexical_hits.iter().enumerate() {
            lexical_rank.insert(hit.chunk_id, index + 1);
            lexical_score.insert(hit.chunk_id, hit.score);
        }

        let mut ids: Vec<Uuid> = vector_rank.keys().copied().collect();
        for id in lexical_rank.keys() {
            if !ids.contains(id) {
                ids.push(*id);
            }
        }
        let chunks = chunk_repo::get_chunks_by_ids(&self.state.pool, &ids).await?;
        let mut chunk_map: HashMap<Uuid, Chunk> =
            chunks.into_iter().map(|chunk| (chunk.id, chunk)).collect();
        let mut hits = Vec::new();
        for chunk_id in ids {
            let Some(chunk) = chunk_map.remove(&chunk_id) else {
                continue;
            };
            let v_rank = vector_rank.get(&chunk_id).copied();
            let l_rank = lexical_rank.get(&chunk_id).copied();
            let mut combined = 0.0f32;
            if let Some(rank) = v_rank {
                combined += 1.0 / (60.0 + rank as f32);
            }
            if let Some(rank) = l_rank {
                combined += 1.0 / (60.0 + rank as f32);
            }
            hits.push(HybridHit {
                chunk_id,
                source_id: Uuid::nil(),
                source_title: "local document".to_owned(),
                text: chunk.text.clone(),
                vector_rank: v_rank,
                lexical_rank: l_rank,
                vector_score: vector_score.get(&chunk_id).copied(),
                lexical_score: lexical_score.get(&chunk_id).copied(),
                combined_score: combined,
                chunk,
            });
        }
        hits.sort_by(|left, right| {
            right
                .combined_score
                .partial_cmp(&left.combined_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        hits.truncate(top_k);
        self.attach_sources(&mut hits).await?;
        Ok(hits)
    }

    async fn vector_hits(
        &self,
        query: &str,
        allowed: &Option<HashSet<Uuid>>,
        limit: usize,
    ) -> Result<Vec<crate::knowledge::vector_index::VectorSearchResult>, AppError> {
        let (config, embedding) = self.state.providers.default_embedding_provider().await?;
        let mut vectors = embedding.embed(vec![query.to_owned()]).await?;
        let query_vector = vectors
            .pop()
            .ok_or_else(|| AppError::Embedding("embedding provider returned no vector".into()))?;
        self.state
            .vector_index
            .search_filtered(
                &query_vector,
                config.id,
                &config.embedding_model,
                allowed.as_ref(),
                limit,
            )
            .await
    }

    async fn allowed_chunk_ids(
        &self,
        filter: &RetrievalFilter,
    ) -> Result<Option<HashSet<Uuid>>, AppError> {
        let mut sets: Vec<HashSet<Uuid>> = Vec::new();

        if let Some(route_id) = filter.route_id {
            let mut route_ids = HashSet::new();
            let course = course_repo::get_course(&self.state.pool, route_id).await?;
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&course.metadata_json) {
                if let Some(values) = value.get("sourceIds").and_then(|value| value.as_array()) {
                    for value in values {
                        if let Some(source_id) =
                            value.as_str().and_then(|v| Uuid::parse_str(v).ok())
                        {
                            route_ids.extend(self.chunk_ids_for_source(source_id).await?);
                        }
                    }
                }
            }
            for node in course_repo::list_nodes(&self.state.pool, route_id).await? {
                route_ids.extend(evidence_ids_from_metadata(&node.metadata_json));
            }
            sets.push(route_ids);
        }

        if let Some(source_ids) = filter.source_ids.as_ref() {
            let mut ids = HashSet::new();
            for source_id in source_ids {
                ids.extend(self.chunk_ids_for_source(*source_id).await?);
            }
            sets.push(ids);
        }

        if let Some(point_ids) = filter.knowledge_point_ids.as_ref() {
            let mut ids = HashSet::new();
            for node in self.all_nodes().await? {
                let points = evidence_ids_from_metadata(&node.metadata_json);
                if points.is_empty() {
                    continue;
                }
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&node.metadata_json) {
                    let point_id = value
                        .get("knowledgePointId")
                        .and_then(|value| value.as_str())
                        .and_then(|value| Uuid::parse_str(value).ok());
                    if let Some(point_id) = point_id {
                        if point_ids.contains(&point_id) {
                            ids.extend(points);
                        }
                    }
                }
            }
            sets.push(ids);
        }

        if sets.is_empty() {
            return Ok(None);
        }
        let mut iter = sets.into_iter();
        let mut intersection = iter.next().unwrap_or_default();
        for set in iter {
            intersection = intersection.intersection(&set).copied().collect();
        }
        Ok(Some(intersection))
    }

    async fn chunk_ids_for_source(&self, source_id: Uuid) -> Result<HashSet<Uuid>, AppError> {
        let ids: Vec<Uuid> = sqlx::query_scalar(
            "SELECT c.id FROM chunks c JOIN documents d ON d.id = c.document_id WHERE d.source_id = ?",
        )
        .bind(source_id)
        .fetch_all(&self.state.pool)
        .await?;
        Ok(ids.into_iter().collect())
    }

    async fn all_nodes(&self) -> Result<Vec<crate::domain::CourseNode>, AppError> {
        sqlx::query_as::<_, crate::domain::CourseNode>(
            "SELECT id, course_id, title, description, objective, order_hint, estimated_minutes, \
             status, metadata_json, lesson_cache_json, lesson_cache_hash, kind, confidence, \
             review_at, generated_by, created_at, updated_at FROM course_nodes",
        )
        .fetch_all(&self.state.pool)
        .await
        .map_err(AppError::from)
    }

    async fn attach_sources(&self, hits: &mut [HybridHit]) -> Result<(), AppError> {
        for hit in hits.iter_mut() {
            let Ok(document) =
                source_repo::get_document(&self.state.pool, hit.chunk.document_id).await
            else {
                continue;
            };
            if let Ok(source) = source_repo::get_source(&self.state.pool, document.source_id).await
            {
                hit.source_id = source.id;
                hit.source_title = source.title;
            }
        }
        Ok(())
    }
}

fn evidence_ids_from_metadata(metadata_json: &str) -> HashSet<Uuid> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(metadata_json) else {
        return HashSet::new();
    };
    value
        .get("evidenceChunkIds")
        .and_then(|value| value.as_array())
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str())
                .filter_map(|value| Uuid::parse_str(value).ok())
                .collect()
        })
        .unwrap_or_default()
}
