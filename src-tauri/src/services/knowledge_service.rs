use crate::ai::prompts::EvidenceSnippet;
use crate::db::repo::{chunks as chunk_repo, sources as source_repo};
use crate::domain::{CourseNode, Source};
use crate::error::AppError;
use crate::knowledge::retrieval::{semantic_search, RetrievedChunk};
use crate::state::AppState;
use serde_json::Value;
use std::collections::HashSet;
use uuid::Uuid;

pub struct KnowledgeService<'a> {
    state: &'a AppState,
}

impl<'a> KnowledgeService<'a> {
    pub fn new(state: &'a AppState) -> Self {
        Self { state }
    }

    pub async fn semantic_search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<RetrievedChunk>, AppError> {
        let (config, embedding) = self.state.providers.default_embedding_provider().await?;
        semantic_search(
            &self.state.pool,
            self.state.vector_index.clone(),
            embedding,
            config.id,
            &config.embedding_model,
            query,
            limit,
        )
        .await
    }

    pub async fn evidence_for_node(
        &self,
        node: &CourseNode,
        limit: usize,
    ) -> Result<Vec<RetrievedChunk>, AppError> {
        let evidence_ids = evidence_chunk_ids(node);
        let mut chunks = chunk_repo::get_chunks_by_ids(&self.state.pool, &evidence_ids).await?;
        let allowed = crate::knowledge::hybrid::HybridRetriever::new(self.state)
            .allowed_chunk_ids(&crate::knowledge::hybrid::RetrievalFilter::for_route(
                node.course_id,
            ))
            .await?;
        if let Some(allowed) = allowed {
            chunks.retain(|chunk| allowed.contains(&chunk.id));
        }
        chunks.truncate(limit);
        if !chunks.is_empty() {
            return Ok(chunks
                .drain(..)
                .map(|chunk| RetrievedChunk { chunk, score: 1.0 })
                .collect());
        }
        use crate::knowledge::hybrid::{HybridRetriever, RetrievalFilter};
        Ok(HybridRetriever::new(self.state)
            .retrieve(
                &format!("{} {}", node.title, node.objective),
                RetrievalFilter::for_route(node.course_id).with_top_k(limit),
            )
            .await?
            .into_iter()
            .map(|hit| hit.into_retrieved())
            .collect())
    }

    /// Evidence restricted to a set of imported sources.
    ///
    /// Used when the learner explicitly attaches material to a new route. The
    /// ranking is a transparent keyword overlap — no embeddings required, so it
    /// still works before any provider is configured.
    pub async fn evidence_for_sources(
        &self,
        source_ids: &[Uuid],
        query: &str,
        limit: usize,
    ) -> Result<Vec<RetrievedChunk>, AppError> {
        if source_ids.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = vec!["?"; source_ids.len()].join(",");
        let sql = format!(
            "SELECT c.id, c.document_id, c.ordinal, c.text, c.char_start, c.char_end, \
             c.page_number, c.heading, c.token_estimate, c.content_hash, c.created_at \
             FROM chunks c JOIN documents d ON d.id = c.document_id \
             WHERE d.source_id IN ({placeholders}) ORDER BY c.ordinal LIMIT ?"
        );
        let mut statement = sqlx::query_as::<_, crate::domain::Chunk>(&sql);
        for id in source_ids {
            statement = statement.bind(id);
        }
        let chunks = statement
            .bind(limit.max(1) as i64 * 4)
            .fetch_all(&self.state.pool)
            .await?;
        Ok(rank_chunks(chunks, query, limit))
    }

    pub async fn source_for_chunk(&self, chunk: &crate::domain::Chunk) -> Result<Source, AppError> {
        let document = source_repo::get_document(&self.state.pool, chunk.document_id).await?;
        source_repo::get_source(&self.state.pool, document.source_id).await
    }

    pub fn evidence_snippets(chunks: &[RetrievedChunk]) -> Vec<EvidenceSnippet> {
        chunks
            .iter()
            .map(|chunk| EvidenceSnippet {
                chunk_id: chunk.chunk.id,
                page_number: chunk.chunk.page_number,
                text: chunk.chunk.text.clone(),
            })
            .collect()
    }
}

pub fn evidence_chunk_ids(node: &CourseNode) -> Vec<Uuid> {
    let Ok(value) = serde_json::from_str::<Value>(&node.metadata_json) else {
        return Vec::new();
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

pub fn evidence_id_set(chunks: &[RetrievedChunk]) -> HashSet<Uuid> {
    chunks.iter().map(|chunk| chunk.chunk.id).collect()
}

/// Rank chunks by literal query overlap, then by brevity.
///
/// Deterministic, inspectable and cheap: good enough to seed a roadmap from a
/// handful of attachments before any embedding provider exists.
pub fn rank_chunks(
    chunks: Vec<crate::domain::Chunk>,
    query: &str,
    limit: usize,
) -> Vec<RetrievedChunk> {
    let needles: Vec<String> = query
        .split(|c: char| c.is_whitespace() || c.is_ascii_punctuation())
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.to_lowercase())
        .collect();

    let mut scored: Vec<RetrievedChunk> = chunks
        .into_iter()
        .map(|chunk| {
            let haystack = chunk.text.to_lowercase();
            let hits = needles
                .iter()
                .filter(|needle| haystack.contains(needle.as_str()))
                .count();
            let coverage = if needles.is_empty() {
                0.0
            } else {
                hits as f32 / needles.len() as f32
            };
            let brevity = 1.0 / (1.0 + chunk.text.len() as f32 / 2000.0);
            RetrievedChunk {
                chunk,
                score: coverage + brevity * 0.2,
            }
        })
        .collect();

    scored.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored.truncate(limit);
    scored
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::time::utc_now;

    fn chunk(text: &str) -> crate::domain::Chunk {
        crate::domain::Chunk {
            id: Uuid::new_v4(),
            document_id: Uuid::new_v4(),
            ordinal: 0,
            text: text.to_owned(),
            char_start: 0,
            char_end: text.len() as i64,
            page_number: None,
            heading: None,
            token_estimate: 1,
            content_hash: "hash".to_owned(),
            created_at: utc_now(),
        }
    }

    #[test]
    fn ranking_prefers_chunks_that_mention_the_topic() {
        let ranked = rank_chunks(
            vec![
                chunk("完全不相关的内容"),
                chunk("射影几何中的无穷远点"),
                chunk("无穷远点是一个方向"),
            ],
            "射影几何 无穷远点",
            2,
        );
        assert_eq!(ranked.len(), 2);
        assert!(ranked[0].score >= ranked[1].score);
        assert!(ranked.iter().any(|hit| hit.chunk.text.contains("无穷远点")));
    }
}
