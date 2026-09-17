use crate::db::repo::{chunks as chunk_repo, knowledge as knowledge_repo, sources as source_repo};
use crate::domain::{
    Chunk, Document, KnowledgeObject, KnowledgeObjectKind, RelationType, Source, SourceKind,
    SourceRevision, SourceStatus,
};
use crate::error::AppError;
use crate::ingest::chunker::{Chunker, HeadingAwareChunker};
use crate::ingest::detector::detect_parser;
use crate::ingest::normalize::normalize_document;
use crate::jobs::manager::JobHandle;
use crate::knowledge_fs::query::source_to_object;
use crate::knowledge_fs::relation::relation;
use crate::state::AppState;
use crate::util::hashing::sha256_hex;
use crate::util::time::utc_now;
use serde_json::json;
use url::Url;
use uuid::Uuid;

const MAX_IMPORT_BYTES: usize = 50 * 1024 * 1024;
const MAX_URL_BYTES: usize = 25 * 1024 * 1024;
const EMBED_BATCH_SIZE: usize = 16;

#[derive(Debug, Clone)]
pub struct IngestInput {
    pub filename: String,
    pub bytes: Vec<u8>,
    pub kind: SourceKind,
    pub title: Option<String>,
    pub uri: Option<String>,
    pub mime_type: Option<String>,
}

pub struct IngestionService<'a> {
    state: &'a AppState,
}

impl<'a> IngestionService<'a> {
    pub fn new(state: &'a AppState) -> Self {
        Self { state }
    }

    pub async fn ingest_bytes(
        &self,
        input: IngestInput,
        handle: Option<&JobHandle>,
    ) -> Result<Source, AppError> {
        if input.bytes.is_empty() {
            return Err(AppError::InvalidInput("cannot import an empty file".into()));
        }
        if input.bytes.len() > MAX_IMPORT_BYTES {
            return Err(AppError::InvalidInput(format!(
                "file too large: {} bytes (limit {MAX_IMPORT_BYTES})",
                input.bytes.len()
            )));
        }
        if let Some(handle) = handle {
            if handle.is_cancelled() {
                return Err(AppError::JobCancelled);
            }
            handle.progress(0.05, "Hashing content").await?;
        }

        let content_hash = sha256_hex(&input.bytes);
        if let Some(existing) =
            source_repo::find_source_by_hash(&self.state.pool, &content_hash).await?
        {
            return Ok(existing);
        }

        if let Some(handle) = handle {
            handle.progress(0.10, "Parsing document").await?;
        }
        let parser = detect_parser(&input.filename, input.mime_type.as_deref())?;
        let parsed = parser.parse(&input.filename, input.mime_type.as_deref(), &input.bytes)?;
        let normalized = normalize_document(parsed.clone());
        let title = input
            .title
            .clone()
            .unwrap_or_else(|| normalized.title.clone());

        if let Some(handle) = handle {
            handle.progress(0.20, "Storing original blob").await?;
        }
        let stored_blob = self.state.blobs.put_bytes(&input.bytes).await?;

        let source_id = Uuid::new_v4();
        let now = utc_now();
        let source = Source {
            id: source_id,
            kind: input.kind,
            title: title.clone(),
            uri: input.uri.clone(),
            original_filename: Some(input.filename.clone()),
            content_hash: None,
            status: SourceStatus::Importing,
            created_at: now,
            updated_at: now,
        };
        source_repo::insert_source(&self.state.pool, &source).await?;

        let revision_no = source_repo::next_revision_number(&self.state.pool, source_id).await?;
        let revision = SourceRevision {
            id: Uuid::new_v4(),
            source_id,
            revision_no,
            content_hash: content_hash.clone(),
            blob_path: stored_blob.relative_path.clone(),
            mime_type: input
                .mime_type
                .clone()
                .unwrap_or_else(|| "application/octet-stream".into()),
            size_bytes: input.bytes.len() as i64,
            created_at: now,
        };
        source_repo::insert_revision(&self.state.pool, &revision).await?;

        let document = Document {
            id: Uuid::new_v4(),
            source_id,
            revision_id: revision.id,
            title: normalized.title.clone(),
            text_content: normalized.full_text(),
            page_count: normalized.page_count(),
            metadata_json: normalized.metadata_json.clone(),
            created_at: now,
        };
        source_repo::insert_document(&self.state.pool, &document).await?;

        if let Some(handle) = handle {
            handle.progress(0.35, "Chunking document").await?;
        }
        let chunks = HeadingAwareChunker::default().chunk(document.id, &normalized)?;
        chunk_repo::insert_chunks(&self.state.pool, &chunks).await?;
        self.index_knowledge_for_document(&source, &document, &chunks)
            .await?;
        source_repo::update_source_status(
            &self.state.pool,
            source_id,
            SourceStatus::Ready,
            Some(&content_hash),
        )
        .await?;

        if let Some(handle) = handle {
            handle.progress(0.55, "Embedding chunks").await?;
        }
        let chunk_ids: Vec<Uuid> = chunks.iter().map(|chunk| chunk.id).collect();
        self.embed_chunks(&chunk_ids, handle).await?;

        source_repo::get_source(&self.state.pool, source_id).await
    }

    pub async fn ingest_url(
        &self,
        url: &str,
        title: Option<String>,
        handle: Option<&JobHandle>,
    ) -> Result<Source, AppError> {
        let parsed = Url::parse(url)
            .map_err(|error| AppError::InvalidInput(format!("invalid URL: {error}")))?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err(AppError::InvalidInput(
                "only http/https URLs can be imported".into(),
            ));
        }
        if let Some(handle) = handle {
            handle.progress(0.02, "Downloading URL").await?;
        }
        let response = self
            .state
            .http
            .get(parsed.clone())
            .send()
            .await
            .map_err(|error| AppError::Network(error.to_string()))?;
        if !response.status().is_success() {
            return Err(AppError::Search(format!(
                "URL import returned {}",
                response.status()
            )));
        }
        if let Some(length) = response.content_length() {
            if length as usize > MAX_URL_BYTES {
                return Err(AppError::InvalidInput(format!(
                    "remote document too large: {length} bytes"
                )));
            }
        }
        let mime_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let bytes = response
            .bytes()
            .await
            .map_err(|error| AppError::Network(error.to_string()))?;
        if bytes.len() > MAX_URL_BYTES {
            return Err(AppError::InvalidInput("remote document too large".into()));
        }
        let filename = parsed
            .path_segments()
            .and_then(|mut segments| segments.next_back())
            .filter(|value| !value.is_empty())
            .unwrap_or("document")
            .to_owned();
        self.ingest_bytes(
            IngestInput {
                filename,
                bytes: bytes.to_vec(),
                kind: SourceKind::Url,
                title,
                uri: Some(url.to_owned()),
                mime_type,
            },
            handle,
        )
        .await
    }

    pub async fn embed_chunks(
        &self,
        chunk_ids: &[Uuid],
        handle: Option<&JobHandle>,
    ) -> Result<(), AppError> {
        if chunk_ids.is_empty() {
            return Ok(());
        }
        let (config, embedding_provider) =
            match self.state.providers.default_embedding_provider().await {
                Ok(value) => value,
                Err(AppError::NotFound(_)) => return Ok(()),
                Err(error) => return Err(error),
            };
        let chunks = chunk_repo::get_chunks_by_ids(&self.state.pool, chunk_ids).await?;
        for (batch_index, batch) in chunks.chunks(EMBED_BATCH_SIZE).enumerate() {
            if let Some(handle) = handle {
                if handle.is_cancelled() {
                    return Err(AppError::JobCancelled);
                }
                let progress = 0.55 + (batch_index as f64 / chunks.len().max(1) as f64) * 0.40;
                handle
                    .progress(
                        progress.min(0.95),
                        format!("Embedding chunk batch {}", batch_index + 1),
                    )
                    .await?;
            }
            let texts: Vec<String> = batch.iter().map(|chunk| chunk.text.clone()).collect();
            let vectors = embedding_provider.embed(texts).await?;
            if vectors.len() != batch.len() {
                return Err(AppError::Embedding(format!(
                    "embedding provider returned {} vectors for {} chunks",
                    vectors.len(),
                    batch.len()
                )));
            }
            for (chunk, vector) in batch.iter().zip(vectors) {
                self.state
                    .vector_index
                    .upsert(chunk.id, config.id, &config.embedding_model, &vector)
                    .await?;
            }
        }
        Ok(())
    }

    pub async fn reindex_source(
        &self,
        source_id: Uuid,
        handle: Option<&JobHandle>,
    ) -> Result<(), AppError> {
        let documents = source_repo::list_documents_by_source(&self.state.pool, source_id).await?;
        let mut chunk_ids = Vec::new();
        for document in documents {
            let chunks = chunk_repo::list_chunks_by_document(&self.state.pool, document.id).await?;
            chunk_ids.extend(chunks.into_iter().map(|chunk| chunk.id));
        }
        self.embed_chunks(&chunk_ids, handle).await
    }

    async fn index_knowledge_for_document(
        &self,
        source: &Source,
        document: &Document,
        chunks: &[Chunk],
    ) -> Result<(), AppError> {
        let source_object = source_to_object(source.clone());
        knowledge_repo::upsert_object(&self.state.pool, &source_object).await?;

        let document_object = KnowledgeObject {
            id: document.id,
            kind: KnowledgeObjectKind::Document,
            title: document.title.clone(),
            description: Some(document.text_content.chars().take(240).collect()),
            metadata_json: json!({
                "sourceId": document.source_id,
                "pageCount": document.page_count,
            })
            .to_string(),
            created_at: document.created_at,
            updated_at: document.created_at,
        };
        knowledge_repo::upsert_object(&self.state.pool, &document_object).await?;

        knowledge_repo::insert_relation(
            &self.state.pool,
            &relation(source.id, document.id, RelationType::Contains, 1.0)?,
        )
        .await?;

        for chunk in chunks {
            let chunk_object = KnowledgeObject {
                id: chunk.id,
                kind: KnowledgeObjectKind::Chunk,
                title: chunk.text.chars().take(80).collect(),
                description: Some(chunk.text.chars().take(240).collect()),
                metadata_json: json!({
                    "documentId": chunk.document_id,
                    "ordinal": chunk.ordinal,
                    "pageNumber": chunk.page_number,
                })
                .to_string(),
                created_at: chunk.created_at,
                updated_at: chunk.created_at,
            };
            knowledge_repo::upsert_object(&self.state.pool, &chunk_object).await?;
            knowledge_repo::insert_relation(
                &self.state.pool,
                &relation(document.id, chunk.id, RelationType::Contains, 1.0)?,
            )
            .await?;
            knowledge_repo::insert_relation(
                &self.state.pool,
                &relation(chunk.id, document.id, RelationType::DerivedFrom, 1.0)?,
            )
            .await?;
            knowledge_repo::insert_relation(
                &self.state.pool,
                &relation(chunk.id, document.id, RelationType::AppearsIn, 1.0)?,
            )
            .await?;
        }
        Ok(())
    }
}
