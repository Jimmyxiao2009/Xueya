use crate::ai::traits::validate_embeddings;
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
        if let Some(handle) = handle {
            handle.progress(0.02, "Downloading URL").await?;
        }

        // A dedicated client with redirects disabled so every hop can be
        // re-validated before the next request.
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("Verruckt/0.1 local source importer")
            .build()
            .map_err(|error| AppError::Internal(error.to_string()))?;
        let fetched = crate::ingest::url_security::fetch_safely(
            &client,
            &parsed,
            crate::ingest::url_security::UrlSecurityPolicy {
                max_bytes: MAX_URL_BYTES,
                ..crate::ingest::url_security::UrlSecurityPolicy::default()
            },
        )
        .await?;
        let filename = fetched
            .final_url
            .path_segments()
            .and_then(|mut segments| segments.next_back())
            .filter(|value| !value.is_empty())
            .unwrap_or("document")
            .to_owned();
        self.ingest_bytes(
            IngestInput {
                filename,
                bytes: fetched.bytes,
                kind: SourceKind::Url,
                title,
                uri: Some(fetched.final_url.to_string()),
                mime_type: fetched.content_type,
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
        if chunks.is_empty() {
            return Ok(());
        }

        // 1. Hash every chunk and reuse vectors from the cache. The cache key
        // includes provider identity, model and dimension config, so identical
        // content under the same embedding space costs zero provider calls.
        let mut keys = Vec::with_capacity(chunks.len());
        let mut vectors: Vec<Option<Vec<f32>>> = Vec::with_capacity(chunks.len());
        for chunk in &chunks {
            let key = crate::knowledge::embedding_cache::cache_key(
                &config,
                &config.embedding_model,
                None,
                &chunk.text,
            );
            let cached =
                crate::knowledge::embedding_cache::get_cached_embedding(&self.state.pool, &key)
                    .await?;
            keys.push(key);
            vectors.push(cached);
        }

        let misses: Vec<usize> = vectors
            .iter()
            .enumerate()
            .filter_map(|(index, vector)| vector.is_none().then_some(index))
            .collect();

        // 2. Embed only cache misses, in batches, with cancellation checks.
        for (batch_index, batch) in misses.chunks(EMBED_BATCH_SIZE).enumerate() {
            if let Some(handle) = handle {
                if handle.is_cancelled() {
                    return Err(AppError::JobCancelled);
                }
                let progress = 0.55 + (batch_index as f64 / misses.len().max(1) as f64) * 0.35;
                handle
                    .progress(
                        progress.min(0.90),
                        format!("Embedding chunk batch {}", batch_index + 1),
                    )
                    .await?;
            }
            let texts: Vec<String> = batch
                .iter()
                .map(|index| chunks[*index].text.clone())
                .collect();
            let embedded = embedding_provider.embed(texts).await?;
            validate_embeddings(&embedded)?;
            if embedded.len() != batch.len() {
                return Err(AppError::Embedding(format!(
                    "embedding provider returned {} vectors for {} chunks",
                    embedded.len(),
                    batch.len()
                )));
            }
            for (offset, vector) in embedded.into_iter().enumerate() {
                let index = batch[offset];
                crate::knowledge::embedding_cache::put_cached_embedding(
                    &self.state.pool,
                    &keys[index],
                    config.id,
                    &config.embedding_model,
                    &vector,
                )
                .await?;
                vectors[index] = Some(vector);
            }
        }

        let final_vectors: Vec<Vec<f32>> = vectors
            .into_iter()
            .enumerate()
            .map(|(index, vector)| {
                vector.ok_or_else(|| {
                    AppError::Embedding(format!("chunk {index} has no embedding after batching"))
                })
            })
            .collect::<Result<_, _>>()?;
        let dimension = validate_embeddings(&final_vectors)?;

        // 3. A provider/model/dimension change invalidates the old index before
        // any new vector is written. This is what makes `needs_reindex` honest.
        crate::knowledge::embedding_cache::reconcile_index_metadata(
            &self.state.pool,
            config.id,
            &config.embedding_model,
            dimension,
        )
        .await?;

        for (chunk, vector) in chunks.iter().zip(final_vectors.iter()) {
            self.state
                .vector_index
                .upsert(chunk.id, config.id, &config.embedding_model, vector)
                .await?;
        }

        // 4. Record which sources are fully indexed under this embedding space.
        let mut document_source_ids = std::collections::HashMap::new();
        for chunk in &chunks {
            if let std::collections::hash_map::Entry::Vacant(entry) =
                document_source_ids.entry(chunk.document_id)
            {
                if let Ok(document) =
                    source_repo::get_document(&self.state.pool, chunk.document_id).await
                {
                    entry.insert(document.source_id);
                }
            }
        }
        for source_id in document_source_ids.values() {
            crate::knowledge::embedding_cache::mark_source_index_ready(
                &self.state.pool,
                *source_id,
                config.id,
                &config.embedding_model,
                dimension,
            )
            .await?;
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
