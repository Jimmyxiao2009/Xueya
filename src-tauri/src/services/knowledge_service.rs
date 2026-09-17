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
        if !chunks.is_empty() {
            return Ok(chunks
                .drain(..)
                .map(|chunk| RetrievedChunk { chunk, score: 1.0 })
                .collect());
        }
        self.semantic_search(&format!("{} {}", node.title, node.objective), limit)
            .await
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
