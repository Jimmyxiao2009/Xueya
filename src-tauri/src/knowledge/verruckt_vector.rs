//! AppData-backed adapter for the standalone Verruckt Vector Engine.
//!
//! The SQLite `embeddings` table remains authoritative. This adapter is
//! derived data: it mirrors cached embeddings into one Exact index per
//! `(provider, model, dimension)` space and can rebuild the files at any
//! time without calling an embedding provider.

use super::embedding::{delete_embeddings_for_chunk, load_all_embeddings, upsert_embedding};
use super::vector_index::{VectorIndex, VectorSearchResult};
use crate::error::AppError;
use crate::util::hashing::sha256_text;
use async_trait::async_trait;
use parking_lot::Mutex;
use sqlx::SqlitePool;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock as AsyncRwLock;
use tokio::task::spawn_blocking;
use uuid::Uuid;
use verruckt_vector::{
    Algorithm, IndexConfig, Metadata, Metric, VectorId, VectorIndex as EngineIndex,
};

const INDEX_FORMAT_ID: &str = "verruckt-vector-v1";
const INDEX_ROOT_NAME: &str = "vector";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SpaceKey {
    provider_id: Uuid,
    model: String,
    dimension: usize,
}

impl SpaceKey {
    fn directory_name(&self) -> String {
        sha256_text(&format!(
            "{INDEX_FORMAT_ID}\n{}\n{}\n{}",
            self.provider_id, self.model, self.dimension
        ))
    }
}

struct Space {
    index: EngineIndex,
}

/// Synchronous engine operations are kept behind a mutex and executed from
/// `spawn_blocking`, so Tauri/Tokio workers never wait on file I/O or CPU
/// traversal. The first integration deliberately uses Exact search: current
/// desktop benchmarks have not established an HNSW crossover yet.
pub struct VerrucktVectorStoreAdapter {
    pool: SqlitePool,
    root: PathBuf,
    spaces: Arc<Mutex<HashMap<SpaceKey, Arc<Mutex<Space>>>>>,
    lifecycle: AsyncRwLock<()>,
}

impl VerrucktVectorStoreAdapter {
    pub async fn new(pool: SqlitePool, app_data_dir: impl Into<PathBuf>) -> Result<Self, AppError> {
        let root = app_data_dir.into().join(INDEX_ROOT_NAME);
        tokio::fs::create_dir_all(&root).await?;
        Ok(Self {
            pool,
            root,
            spaces: Arc::new(Mutex::new(HashMap::new())),
            lifecycle: AsyncRwLock::new(()),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    async fn open_space(&self, key: SpaceKey) -> Result<Arc<Mutex<Space>>, AppError> {
        if let Some(existing) = self.spaces.lock().get(&key).cloned() {
            return Ok(existing);
        }
        let spaces = Arc::clone(&self.spaces);
        let root = self.root.clone();
        let key_for_open = key.clone();
        let opened = spawn_blocking(move || open_or_create_space(&root, key_for_open))
            .await
            .map_err(|error| AppError::Internal(format!("vector open task failed: {error}")))??;
        let opened = Arc::new(Mutex::new(opened));
        let mut guard = spaces.lock();
        Ok(guard
            .entry(key)
            .or_insert_with(|| Arc::clone(&opened))
            .clone())
    }

    async fn space_for_vector(
        &self,
        provider_id: Uuid,
        model: &str,
        vector: &[f32],
    ) -> Result<Arc<Mutex<Space>>, AppError> {
        if vector.is_empty() || vector.iter().any(|value| !value.is_finite()) {
            return Err(AppError::Embedding("invalid vector".into()));
        }
        self.open_space(SpaceKey {
            provider_id,
            model: model.to_owned(),
            dimension: vector.len(),
        })
        .await
    }

    async fn flush_all(&self) -> Result<(), AppError> {
        let _lifecycle = self.lifecycle.write().await;
        let spaces: Vec<_> = self.spaces.lock().values().cloned().collect();
        for space in spaces {
            let result = spawn_blocking(move || {
                let guard = space.lock();
                guard.index.flush().map_err(engine_error)
            })
            .await
            .map_err(|error| AppError::Internal(format!("vector flush task failed: {error}")))?;
            result?;
        }
        Ok(())
    }

    async fn rebuild_from_sqlite(&self) -> Result<(), AppError> {
        let records = load_all_embeddings(&self.pool).await?;
        let mut grouped: HashMap<SpaceKey, Vec<(Uuid, Vec<f32>)>> = HashMap::new();
        for record in records {
            if record.dimensions <= 0 || record.vector.len() != record.dimensions as usize {
                continue;
            }
            grouped
                .entry(SpaceKey {
                    provider_id: record.provider_id,
                    model: record.model,
                    dimension: record.dimensions as usize,
                })
                .or_default()
                .push((record.chunk_id, record.vector));
        }

        self.spaces.lock().clear();
        remove_derived_directories(&self.root)?;
        for (key, records) in grouped {
            let root = self.root.clone();
            let build_key = key.clone();
            let built = spawn_blocking(move || {
                let path = root.join(build_key.directory_name());
                std::fs::create_dir_all(&path)?;
                let index = EngineIndex::create(&path, engine_config(build_key.dimension))
                    .map_err(engine_error)?;
                for (id, vector) in records {
                    index
                        .insert(VectorId::from_uuid(id), &vector, Metadata::empty())
                        .map_err(engine_error)?;
                }
                index.flush().map_err(engine_error)?;
                Ok::<Space, AppError>(Space { index })
            })
            .await
            .map_err(|error| {
                AppError::Internal(format!("vector rebuild task failed: {error}"))
            })??;
            self.spaces.lock().insert(key, Arc::new(Mutex::new(built)));
        }
        Ok(())
    }

    async fn search_space(
        &self,
        key: SpaceKey,
        query: &[f32],
        limit: usize,
    ) -> Result<Vec<VectorSearchResult>, AppError> {
        let space = self.open_space(key).await?;
        let query = query.to_vec();
        spawn_blocking(move || {
            let hits = space
                .lock()
                .index
                .search(&query, limit.max(1))
                .map_err(engine_error)?;
            Ok(hits
                .into_iter()
                .map(|hit| VectorSearchResult {
                    chunk_id: hit.id.to_uuid(),
                    score: hit.score,
                })
                .collect())
        })
        .await
        .map_err(|error| AppError::Internal(format!("vector search task failed: {error}")))?
    }
}

#[async_trait]
impl VectorIndex for VerrucktVectorStoreAdapter {
    async fn upsert(
        &self,
        chunk_id: Uuid,
        provider_id: Uuid,
        model: &str,
        vector: &[f32],
    ) -> Result<(), AppError> {
        let _lifecycle = self.lifecycle.read().await;
        // Preserve the existing SQLite/cache contract first. If this fails,
        // the derived index is not changed.
        upsert_embedding(&self.pool, chunk_id, provider_id, model, vector).await?;
        let space = self.space_for_vector(provider_id, model, vector).await?;
        let id = VectorId::from_uuid(chunk_id);
        let vector = vector.to_vec();
        spawn_blocking(move || {
            space
                .lock()
                .index
                .insert(id, &vector, Metadata::empty())
                .map_err(engine_error)
        })
        .await
        .map_err(|error| AppError::Internal(format!("vector upsert task failed: {error}")))??;
        Ok(())
    }

    async fn delete(&self, chunk_id: Uuid) -> Result<(), AppError> {
        let _lifecycle = self.lifecycle.read().await;
        // Authoritative deletion remains SQLite; derived tombstoning is best
        // effort across all already-open spaces.
        delete_embeddings_for_chunk(&self.pool, chunk_id).await?;
        let spaces: Vec<_> = self.spaces.lock().values().cloned().collect();
        for space in spaces {
            let id = VectorId::from_uuid(chunk_id);
            spawn_blocking(move || space.lock().index.delete(id).map_err(engine_error))
                .await
                .map_err(|error| {
                    AppError::Internal(format!("vector delete task failed: {error}"))
                })??;
        }
        Ok(())
    }

    async fn search(
        &self,
        query: &[f32],
        provider_id: Uuid,
        model: &str,
        limit: usize,
    ) -> Result<Vec<VectorSearchResult>, AppError> {
        let _lifecycle = self.lifecycle.read().await;
        if query.is_empty() || query.iter().any(|value| !value.is_finite()) {
            return Err(AppError::Embedding("invalid query embedding".into()));
        }
        let key = SpaceKey {
            provider_id,
            model: model.to_owned(),
            dimension: query.len(),
        };
        let space = self.open_space(key).await?;
        let query = query.to_vec();
        let limit = limit.clamp(1, 100);
        spawn_blocking(move || {
            let hits = space
                .lock()
                .index
                .search(&query, limit)
                .map_err(engine_error)?;
            Ok(hits
                .into_iter()
                .map(|hit| VectorSearchResult {
                    chunk_id: hit.id.to_uuid(),
                    score: hit.score,
                })
                .collect())
        })
        .await
        .map_err(|error| AppError::Internal(format!("vector search task failed: {error}")))?
    }

    async fn search_filtered(
        &self,
        query: &[f32],
        provider_id: Uuid,
        model: &str,
        allowed_chunk_ids: Option<&HashSet<Uuid>>,
        limit: usize,
    ) -> Result<Vec<VectorSearchResult>, AppError> {
        let _lifecycle = self.lifecycle.read().await;
        if query.is_empty() || query.iter().any(|value| !value.is_finite()) {
            return Err(AppError::Embedding("invalid query embedding".into()));
        }
        // Exact is intentional here: the production trait accepts an arbitrary
        // chunk-ID set, which cannot be represented by typed metadata
        // predicates without losing route/evidence correctness.
        let overfetch = allowed_chunk_ids
            .map(|ids| ids.len().max(limit).max(1))
            .unwrap_or(limit.clamp(1, 100));
        let hits = self
            .search_space(
                SpaceKey {
                    provider_id,
                    model: model.to_owned(),
                    dimension: query.len(),
                },
                query,
                overfetch,
            )
            .await?;
        Ok(hits
            .into_iter()
            .filter(|hit| {
                allowed_chunk_ids
                    .map(|allowed| allowed.contains(&hit.chunk_id))
                    .unwrap_or(true)
            })
            .take(limit.clamp(1, 100))
            .collect())
    }

    async fn rebuild(&self) -> Result<(), AppError> {
        self.rebuild_from_sqlite().await
    }

    async fn flush(&self) -> Result<(), AppError> {
        self.flush_all().await
    }

    async fn info(&self) -> Option<super::vector_index::VectorBackendStats> {
        // Aggregate over the open spaces; a desktop app normally has one.
        let first = self.spaces.lock().values().next().cloned();
        let first = first?;
        let stats = spawn_blocking(move || {
            let guard = first.lock();
            guard.index.stats().ok()
        })
        .await
        .ok()??;
        Some(super::vector_index::VectorBackendStats {
            algorithm: format!("{:?}", stats.algorithm).to_lowercase(),
            vector_count: stats.live_count,
            deleted_count: Some(stats.deleted_count),
            dimension: stats.dimensions,
            index_bytes: Some(stats.disk_bytes),
        })
    }
}

fn engine_config(dimension: usize) -> IndexConfig {
    let mut config = IndexConfig::new(dimension as u32, Metric::Cosine);
    config.algorithm = Algorithm::Exact;
    config.exact_threshold = u64::MAX;
    config
}

fn open_or_create_space(root: &Path, key: SpaceKey) -> Result<Space, AppError> {
    let path = root.join(key.directory_name());
    std::fs::create_dir_all(root)?;
    let index = match EngineIndex::open(&path) {
        Ok(index) => index,
        Err(verruckt_vector::VectorError::IndexNotFound { .. }) => {
            std::fs::create_dir_all(&path)?;
            EngineIndex::create(&path, engine_config(key.dimension)).map_err(engine_error)?
        }
        Err(verruckt_vector::VectorError::CorruptIndex { .. })
        | Err(verruckt_vector::VectorError::UnsupportedVersion { .. }) => {
            tracing::warn!(path = %path.display(), "derived vector index is invalid; rebuilding from SQLite embeddings");
            std::fs::remove_dir_all(&path)?;
            std::fs::create_dir_all(&path)?;
            EngineIndex::create(&path, engine_config(key.dimension)).map_err(engine_error)?
        }
        Err(error) => return Err(engine_error(error)),
    };
    Ok(Space { index })
}

fn remove_derived_directories(root: &Path) -> Result<(), AppError> {
    if !root.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            std::fs::remove_dir_all(entry.path())?;
        }
    }
    Ok(())
}

fn engine_error(error: verruckt_vector::VectorError) -> AppError {
    match error {
        verruckt_vector::VectorError::InvalidDimension { .. }
        | verruckt_vector::VectorError::InvalidVector { .. }
        | verruckt_vector::VectorError::InvalidMetadata { .. }
        | verruckt_vector::VectorError::InvalidArgument { .. }
        | verruckt_vector::VectorError::ConfigMismatch { .. } => {
            AppError::Embedding(error.to_string())
        }
        verruckt_vector::VectorError::IndexNotFound { .. }
        | verruckt_vector::VectorError::IndexAlreadyExists { .. }
        | verruckt_vector::VectorError::UnsupportedVersion { .. }
        | verruckt_vector::VectorError::CorruptIndex { .. }
        | verruckt_vector::VectorError::IndexClosed
        | verruckt_vector::VectorError::Io(_) => AppError::Io(error.to_string()),
    }
}
