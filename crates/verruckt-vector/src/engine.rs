//! The public embedded-engine API.
//!
//! [`VectorIndex`] is a directory of files: `create` initializes it, `open`
//! reopens it (checkpoint + WAL replay), and every mutation is WAL-logged
//! before it touches memory. Search never blocks on writers longer than one
//! lock acquisition — searches take the read lock and see a consistent
//! snapshot of rows + graph.
//!
//! Concurrency model (documented in README §Concurrency):
//!
//! * one writer lock for insert/delete/flush/compact — writers serialize;
//! * many concurrent readers — search takes the read lock;
//! * a search concurrent with a write sees either the pre- or post-write
//!   state, never a half-applied mutation, and never panics.
//!
//! Score semantics are uniform: **higher = more similar** (Cosine in
//! `[-1, 1]`, Dot raw, L2 as `-distance`).

use crate::config::{Algorithm, IndexConfig, Metric};
use crate::error::{Result, VectorError};
use crate::exact::exact_search;
use crate::hnsw::{self, graph_insert, graph_search, SearchCtx};
use crate::id::VectorId;
use crate::metadata::{Metadata, MetadataFilter};
use crate::persist::{
    self, cleanup_stale_segments, config_from_manifest, directory_size, load_segments,
    manifest_path, wal_path, Manifest,
};
use crate::storage::{Rows, VectorData};
use crate::wal::{replay_wal, truncate_to, WalRecord, WalWriter};
use parking_lot::{Mutex, RwLock};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// A search hit. `score` is higher = more similar; `metadata` is the row's
/// stored metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchResult {
    pub id: VectorId,
    pub score: f32,
    pub metadata: Metadata,
}

/// Engine statistics snapshot.
#[derive(Debug, Clone, PartialEq)]
pub struct IndexStats {
    pub dimensions: u32,
    pub metric: Metric,
    pub algorithm: Algorithm,
    pub live_count: u64,
    pub deleted_count: u64,
    pub row_count: u64,
    pub wal_bytes: u64,
    pub generation: u64,
    pub max_seq: u64,
    pub disk_bytes: u64,
    /// Approximate engine heap usage (vector arena + bookkeeping), excluding
    /// file-backed mmap pages.
    pub heap_bytes: u64,
    pub should_compact: bool,
    /// True when the vectors segment is mmap-backed (fast startup mode).
    pub mmap: bool,
}

/// Tunables that live outside `IndexConfig` (they affect runtime behavior,
/// not the on-disk identity of the index).
#[derive(Debug, Clone)]
pub struct EngineOptions {
    /// Memory-map the checkpointed vectors segment instead of copying it to
    /// the heap (faster startup, lower heap). Default: true.
    pub use_mmap: bool,
    /// Fsync the WAL on every mutation (durable, slower) or only on
    /// flush/checkpoint (fast, crash may lose the tail since the last
    /// flush — never corrupts). Default: false.
    pub sync_wal_per_op: bool,
    /// Checkpoint automatically once the WAL exceeds this many bytes.
    /// Default: taken from `IndexConfig::persistence`.
    pub wal_auto_checkpoint_bytes: Option<u64>,
}

impl Default for EngineOptions {
    fn default() -> Self {
        EngineOptions {
            use_mmap: true,
            sync_wal_per_op: false,
            wal_auto_checkpoint_bytes: None,
        }
    }
}

struct Inner {
    dir: PathBuf,
    config: IndexConfig,
    rows: Rows,
    graph: hnsw::HnswGraph,
    wal: WalWriter,
    generation: u64,
    max_seq: u64,
    created_at_unix: i64,
    mmap: bool,
    /// Sequence numbers replayed from a torn WAL tail are reused, not gap-
    /// skipped: after truncation the next op rewrites the same seq.
    next_seq: AtomicU64,
    closed: AtomicBool,
    sync_wal_per_op: bool,
    wal_auto_checkpoint_bytes: u64,
}

impl Inner {
    fn config(&self) -> IndexConfig {
        self.config.clone()
    }

    fn dim(&self) -> usize {
        self.config.dimensions as usize
    }

    fn checkpoint_locked(&mut self) -> Result<()> {
        let generation = self.generation + 1;
        let dim = self.dim();
        let checksums =
            persist::write_segments(&self.dir, generation, &self.rows, &self.graph, dim)?;

        let manifest = Manifest {
            format_version: crate::codec::frame::FORMAT_VERSION,
            generation,
            dimensions: self.config.dimensions,
            metric: self.config.metric,
            algorithm: self.config.algorithm,
            m: self.config.hnsw.m,
            ef_construction: self.config.hnsw.ef_construction,
            ef_search: self.config.hnsw.ef_search,
            heuristic: self.config.hnsw.heuristic,
            seed: self.config.hnsw.seed,
            exact_threshold: self.config.exact_threshold,
            row_count: self.rows.row_count() as u32,
            live_count: self.rows.live_count,
            deleted_count: self.rows.deleted_count,
            max_seq: self.max_seq,
            graph_entry: self.graph.entry(),
            graph_max_level: self.graph.max_level(),
            vectors_checksum: checksums.vectors,
            graph_checksum: checksums.graph,
            ids_checksum: checksums.ids,
            meta_checksum: checksums.meta,
            created_at_unix: self.created_at_unix,
            embedding_space: self.config.embedding_space.clone(),
        };
        // The manifest rename is the commit point of this generation.
        crate::codec::atomic_write(&self.dir, "manifest.bin", &manifest.encode())?;
        self.wal.rotate()?;
        self.generation = generation;
        cleanup_stale_segments(&self.dir, generation)?;
        Ok(())
    }

    fn maybe_auto_checkpoint(&mut self, threshold_bytes: u64) -> Result<()> {
        if self.wal.bytes_since_checkpoint >= threshold_bytes {
            self.checkpoint_locked()?;
        }
        Ok(())
    }
}

/// An open vector index. Clone-free handle: wrap in `Arc` to share.
pub struct VectorIndex {
    inner: RwLock<Inner>,
    /// Serializes writers only (the RwLock already serializes everything,
    /// but an explicit writer mutex keeps the WAL-append → mutate critical
    /// section obvious and future-proof).
    write_lock: Mutex<()>,
}

impl std::fmt::Debug for VectorIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VectorIndex")
            .field("dir", &self.inner.read().dir)
            .finish_non_exhaustive()
    }
}

impl VectorIndex {
    /// Create a new index at `path`. Fails if the directory already contains
    /// a manifest (use [`VectorIndex::open`]).
    pub fn create(path: &Path, config: IndexConfig) -> Result<VectorIndex> {
        config.validate()?;
        if manifest_path(path).exists() {
            return Err(VectorError::IndexAlreadyExists {
                path: path.display().to_string(),
            });
        }
        std::fs::create_dir_all(path)?;

        let inner = Inner {
            dir: path.to_path_buf(),
            config: config.clone(),
            rows: Rows::empty(VectorData::empty_heap()),
            graph: hnsw::HnswGraph::new(config.hnsw.m),
            wal: WalWriter::create(path)?,
            generation: 0,
            max_seq: 0,
            created_at_unix: now_unix(),
            mmap: false,
            next_seq: AtomicU64::new(1),
            closed: AtomicBool::new(false),
            sync_wal_per_op: false,
            wal_auto_checkpoint_bytes: config.persistence.wal_auto_checkpoint_bytes,
        };
        let mut inner = inner;
        // Persist generation 0 immediately so a freshly created index is a
        // complete, openable on-disk state.
        inner.checkpoint_locked()?;

        Ok(VectorIndex {
            inner: RwLock::new(inner),
            write_lock: Mutex::new(()),
        })
    }

    /// Open an existing index: load the checkpointed generation, verify all
    /// segment checksums, then replay the WAL on top.
    pub fn open(path: &Path) -> Result<VectorIndex> {
        Self::open_with(path, &EngineOptions::default())
    }

    /// Open with runtime durability, checkpoint and mmap options.
    pub fn open_with(path: &Path, options: &EngineOptions) -> Result<VectorIndex> {
        let manifest_bytes =
            std::fs::read(manifest_path(path)).map_err(|_| VectorError::IndexNotFound {
                path: path.display().to_string(),
            })?;
        let manifest = Manifest::decode(&manifest_bytes)?;
        manifest.verify_checksums(path)?;

        let config = config_from_manifest(&manifest);
        let use_mmap = options.use_mmap;
        let wal_auto_checkpoint_bytes = options
            .wal_auto_checkpoint_bytes
            .unwrap_or(config.persistence.wal_auto_checkpoint_bytes);
        let (mut rows, mut graph) = load_segments(path, &manifest, use_mmap)?;

        // Replay the WAL. A torn tail is truncated; mid-log damage is an error.
        let wal_bytes = std::fs::read(wal_path(path)).unwrap_or_default();
        let replay = replay_wal(&wal_bytes)?;
        if let Some(torn_at) = replay.torn_at {
            truncate_to(&wal_path(path), torn_at)?;
        }
        let mut max_seq = manifest.max_seq;
        let mut next_seq = max_seq + 1;
        let dim = config.dimensions as usize;
        for record in &replay.records {
            match record {
                WalRecord::Insert {
                    seq,
                    id,
                    vector,
                    metadata,
                } => {
                    let row = rows.insert(*id, vector, metadata.clone());
                    insert_into_graph(&mut graph, &rows, row, *seq, &config, dim);
                    max_seq = (*seq).max(max_seq);
                    next_seq = (*seq) + 1;
                }
                WalRecord::Delete { seq, id } => {
                    if let Some(&row) = rows.idmap.get(id) {
                        rows.tombstone(row);
                    }
                    max_seq = (*seq).max(max_seq);
                    next_seq = (*seq) + 1;
                }
            }
        }
        // Counters are recomputed from the alive markers, never trusted.
        let wal = WalWriter::create(path)?;

        Ok(VectorIndex {
            inner: RwLock::new(Inner {
                dir: path.to_path_buf(),
                config,
                rows,
                graph,
                wal,
                generation: manifest.generation,
                max_seq,
                created_at_unix: manifest.created_at_unix,
                mmap: use_mmap,
                next_seq: AtomicU64::new(next_seq),
                closed: AtomicBool::new(false),
                sync_wal_per_op: options.sync_wal_per_op,
                wal_auto_checkpoint_bytes,
            }),
            write_lock: Mutex::new(()),
        })
    }

    /// Insert one vector (upsert: an existing live row with the same id is
    /// tombstoned and re-inserted).
    pub fn insert(&self, id: VectorId, vector: &[f32], metadata: Metadata) -> Result<()> {
        self.insert_batch(std::iter::once((id, vector, metadata)))
    }

    /// Insert many vectors in one WAL run (embeddings arrive in batches).
    /// Each vector is validated and normalized (Cosine) before anything is
    /// appended; the batch either fully applies or nothing does.
    pub fn insert_batch<'v, I>(&self, batch: I) -> Result<()>
    where
        I: IntoIterator<Item = (VectorId, &'v [f32], Metadata)>,
    {
        let _guard = self.write_lock.lock();
        let mut inner = self.inner.write();
        if inner.closed.load(Ordering::Acquire) {
            return Err(VectorError::IndexClosed);
        }
        let dim = inner.dim();
        let sync = inner.sync_wal_per_op;
        let threshold = inner.wal_auto_checkpoint_bytes;

        let mut pending: Vec<(VectorId, Vec<f32>, Metadata)> = Vec::new();
        for (id, vector, metadata) in batch {
            let prepared = prepare_vector(&inner.config, dim, vector)?;
            metadata.validate()?;
            pending.push((id, prepared, metadata));
        }

        for (id, vector, metadata) in pending {
            let seq = inner.next_seq.fetch_add(1, Ordering::AcqRel);
            inner.wal.append(
                &WalRecord::Insert {
                    seq,
                    id,
                    vector: vector.clone(),
                    metadata: metadata.clone(),
                },
                sync,
            )?;
            let row = inner.rows.insert(id, &vector, metadata);
            let config = inner.config.clone();
            let mut graph = std::mem::replace(&mut inner.graph, hnsw::HnswGraph::new(1));
            insert_into_graph(&mut graph, &inner.rows, row, seq, &config, dim);
            inner.graph = graph;
            inner.max_seq = seq.max(inner.max_seq);
        }

        inner.maybe_auto_checkpoint(threshold)
    }

    /// Delete (tombstone) a vector. Deleting an unknown id is a no-op for
    /// compatibility with the mainline `VectorIndex::delete` contract.
    pub fn delete(&self, id: VectorId) -> Result<()> {
        let _guard = self.write_lock.lock();
        let mut inner = self.inner.write();
        if inner.closed.load(Ordering::Acquire) {
            return Err(VectorError::IndexClosed);
        }
        if !inner.rows.idmap.contains_key(&id) {
            return Ok(());
        }
        let seq = inner.next_seq.fetch_add(1, Ordering::AcqRel);
        let sync = inner.sync_wal_per_op;
        inner.wal.append(&WalRecord::Delete { seq, id }, sync)?;
        if let Some(&row) = inner.rows.idmap.get(&id) {
            inner.rows.tombstone(row);
        }
        inner.max_seq = seq.max(inner.max_seq);
        let threshold = inner.wal_auto_checkpoint_bytes;
        inner.maybe_auto_checkpoint(threshold)
    }

    /// Search for the top-`k` nearest vectors.
    pub fn search(&self, query: &[f32], k: usize) -> Result<Vec<SearchResult>> {
        self.search_filtered(query, k, None)
    }

    /// Search with a metadata filter.
    pub fn search_filtered(
        &self,
        query: &[f32],
        k: usize,
        filter: Option<&MetadataFilter>,
    ) -> Result<Vec<SearchResult>> {
        if k == 0 {
            return Err(VectorError::invalid_argument("k must be >= 1"));
        }
        let inner = self.inner.read();
        if inner.closed.load(Ordering::Acquire) {
            return Err(VectorError::IndexClosed);
        }
        let dim = inner.dim();
        if query.len() != dim {
            return Err(VectorError::InvalidDimension {
                expected: inner.config.dimensions,
                got: query.len(),
            });
        }
        let prepared = prepare_query(&inner.config, query)?;

        let live = inner.rows.live_count;
        let use_exact = match inner.config.algorithm {
            Algorithm::Exact => true,
            Algorithm::Hnsw => false,
            Algorithm::Adaptive => live <= inner.config.exact_threshold,
        };

        let hits = if use_exact {
            exact_search(&inner.rows, dim, inner.config.metric, &prepared, k, filter)
        } else {
            hnsw_filtered_search(&inner, dim, &prepared, k, filter)
        };

        Ok(hits
            .into_iter()
            .map(|hit| SearchResult {
                id: inner.rows.ids[hit.row as usize],
                score: hit.score,
                metadata: inner.rows.meta[hit.row as usize].clone(),
            })
            .collect())
    }

    /// Full durable flush: rewrite all segments at a new generation, commit
    /// by manifest rename, fsync the WAL boundary, and rotate the WAL.
    pub fn flush(&self) -> Result<()> {
        let _guard = self.write_lock.lock();
        let mut inner = self.inner.write();
        if inner.closed.load(Ordering::Acquire) {
            return Err(VectorError::IndexClosed);
        }
        inner.checkpoint_locked()
    }

    /// Full checkpoint: rewrite all segments at a new generation and commit
    /// by manifest rename. Safe to call at any time; also truncates the WAL.
    pub fn compact(&self) -> Result<()> {
        let _guard = self.write_lock.lock();
        let mut inner = self.inner.write();
        if inner.closed.load(Ordering::Acquire) {
            return Err(VectorError::IndexClosed);
        }
        // Compaction: rebuild dense rows (drop tombstones), then checkpoint.
        compact_locked(&mut inner)?;
        inner.checkpoint_locked()
    }

    /// Statistics snapshot.
    pub fn stats(&self) -> Result<IndexStats> {
        let inner = self.inner.read();
        let config = inner.config();
        let wal_bytes = std::fs::metadata(wal_path(&inner.dir))
            .map(|meta| meta.len())
            .unwrap_or(0);
        let deleted = inner.rows.deleted_count;
        let total = inner.rows.live_count + deleted;
        Ok(IndexStats {
            dimensions: config.dimensions,
            metric: config.metric,
            algorithm: config.algorithm,
            live_count: inner.rows.live_count,
            deleted_count: deleted,
            row_count: inner.rows.row_count() as u64,
            wal_bytes,
            generation: inner.generation,
            max_seq: inner.max_seq,
            disk_bytes: directory_size(&inner.dir),
            heap_bytes: inner.rows.heap_bytes() + inner.graph.links_bytes(),
            should_compact: should_compact(&config, inner.rows.live_count, deleted, total),
            mmap: inner.mmap,
        })
    }

    /// The index's recorded configuration.
    pub fn config(&self) -> IndexConfig {
        self.inner.read().config()
    }

    /// True when compaction is advisable (tombstone ratio above threshold).
    pub fn should_compact(&self) -> Result<bool> {
        Ok(self.stats()?.should_compact)
    }

    /// Flush the WAL and mark closed. Dropping the handle also closes it.
    pub fn close(&self) -> Result<()> {
        let _guard = self.write_lock.lock();
        let mut inner = self.inner.write();
        if inner.closed.load(Ordering::Acquire) {
            return Ok(());
        }
        let result = inner.wal.sync();
        inner.closed.store(true, Ordering::Release);
        result
    }
}

/// Connect `row` into the HNSW graph with the deterministic level for `seq`.
/// Shared by live inserts, WAL replay, and compaction.
fn insert_into_graph(
    graph: &mut hnsw::HnswGraph,
    rows: &Rows,
    row: u32,
    seq: u64,
    config: &IndexConfig,
    dim: usize,
) {
    let level = hnsw::assign_level(config.hnsw.seed, seq, config.hnsw.m);
    let ctx = SearchCtx {
        vectors: &rows.vectors,
        meta: &rows.meta,
        alive: &rows.alive,
        dim,
        metric: config.metric,
        query: rows.vectors.row(row as usize, dim),
        filter: None,
    };
    graph_insert(
        graph,
        &ctx,
        row,
        level,
        config.hnsw.ef_construction as usize,
        config.hnsw.heuristic,
    );
}

/// Filtered HNSW search with bounded ef escalation (see README §Filtering):
/// try the configured ef; if too few results survive the filter, retry with
/// a larger ef, up to a final exact fallback for pathological filters.
fn hnsw_filtered_search(
    inner: &Inner,
    dim: usize,
    query: &[f32],
    k: usize,
    filter: Option<&MetadataFilter>,
) -> Vec<crate::exact::Scored> {
    let base_ef = inner.config.hnsw.ef_search as usize;
    let mut ef = base_ef.max(k);
    // Up to three escalation attempts: ×4, ×16, then exact fallback.
    for _ in 0..3 {
        let ctx = SearchCtx {
            vectors: &inner.rows.vectors,
            meta: &inner.rows.meta,
            alive: &inner.rows.alive,
            dim,
            metric: inner.config.metric,
            query,
            filter,
        };
        let hits = graph_search(&ctx, &inner.graph, k, ef, None);
        if filter.is_none() || hits.len() >= k {
            return hits
                .into_iter()
                .map(|hit| crate::exact::Scored {
                    score: crate::distance::score(
                        inner.config.metric,
                        query,
                        inner.rows.vectors.row(hit.node as usize, dim),
                    ),
                    row: hit.node,
                })
                .collect();
        }
        ef = ef.saturating_mul(4);
    }
    // Selective filter defeated the graph search: exact fallback.
    exact_search(&inner.rows, dim, inner.config.metric, query, k, filter)
}

/// Drop tombstoned rows and rebuild dense vectors/ids/meta/graph.
fn compact_locked(inner: &mut Inner) -> Result<()> {
    let dim = inner.dim();
    let config = inner.config.clone();
    let live_rows: Vec<u32> = (0..inner.rows.row_count() as u32)
        .filter(|&row| inner.rows.alive[row as usize])
        .collect();

    let mut new_rows = Rows::empty(VectorData::empty_heap());
    let mut new_graph = hnsw::HnswGraph::new(config.hnsw.m);
    // Preserve insertion order; seq starts at 1 so level assignment stays
    // deterministic for a given live set.
    for (position, &row) in live_rows.iter().enumerate() {
        let id = inner.rows.ids[row as usize];
        let vector = inner.rows.copy_vector(row, dim);
        let metadata = inner.rows.meta[row as usize].clone();
        let seq = (position + 1) as u64;
        let new_row = new_rows.insert(id, &vector, metadata);
        insert_into_graph(&mut new_graph, &new_rows, new_row, seq, &config, dim);
    }
    inner.rows = new_rows;
    inner.graph = new_graph;
    Ok(())
}

fn should_compact(config: &IndexConfig, live: u64, deleted: u64, total: u64) -> bool {
    if total == 0 {
        return false;
    }
    deleted * 10 >= total * (config.persistence.compact_ratio_threshold * 10.0) as u64
        && deleted > 0
        && live + deleted > 0
}

/// Validate, and for Cosine normalize, an inserted vector.
fn prepare_vector(config: &IndexConfig, dim: usize, vector: &[f32]) -> Result<Vec<f32>> {
    if vector.len() != dim {
        return Err(VectorError::InvalidDimension {
            expected: config.dimensions,
            got: vector.len(),
        });
    }
    if vector.is_empty() {
        return Err(VectorError::invalid_vector("vector is empty"));
    }
    if vector.iter().any(|component| !component.is_finite()) {
        return Err(VectorError::invalid_vector(
            "vector contains non-finite values",
        ));
    }
    let mut prepared = vector.to_vec();
    if config.metric == Metric::Cosine && !crate::distance::normalize_in_place(&mut prepared) {
        return Err(VectorError::invalid_vector(
            "zero vector cannot be normalized for cosine metric",
        ));
    }
    Ok(prepared)
}

/// Validate and (for Cosine) normalize a query vector.
fn prepare_query(config: &IndexConfig, query: &[f32]) -> Result<Vec<f32>> {
    prepare_vector(config, config.dimensions as usize, query)
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config(dim: u32) -> IndexConfig {
        IndexConfig::new(dim, Metric::Cosine)
    }

    /// Nearly-orthogonal unit vectors in dim-space (a rotation-free
    /// construction: axis i gets a strong component, others weak), so cosine
    /// similarity meaningfully separates different seeds.
    fn vec_of(seed: usize, dim: usize) -> Vec<f32> {
        let mut v = vec![0.01; dim];
        v[seed % dim] = 1.0;
        v
    }

    #[test]
    fn create_open_search_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let index = VectorIndex::create(dir.path(), test_config(8)).unwrap();
        let ids: Vec<VectorId> = (0..10).map(|i| VectorId(i as u128 + 1)).collect();
        for (i, id) in ids.iter().enumerate() {
            index.insert(*id, &vec_of(i, 8), Metadata::empty()).unwrap();
        }
        index.flush().unwrap();

        // Reopen and search: results must be identical.
        let reopened = VectorIndex::open(dir.path()).unwrap();
        let query = vec_of(3, 8);
        let before: Vec<VectorId> = index
            .search(&query, 5)
            .unwrap()
            .into_iter()
            .map(|hit| hit.id)
            .collect();
        let after: Vec<VectorId> = reopened
            .search(&query, 5)
            .unwrap()
            .into_iter()
            .map(|hit| hit.id)
            .collect();
        assert_eq!(before, after);
        assert!(before.contains(&VectorId(4))); // nearest to seed 3
    }

    #[test]
    fn create_twice_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        VectorIndex::create(dir.path(), test_config(4)).unwrap();
        let err = VectorIndex::create(dir.path(), test_config(4)).unwrap_err();
        assert!(matches!(err, VectorError::IndexAlreadyExists { .. }));
    }

    #[test]
    fn open_missing_directory_is_index_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let err = VectorIndex::open(&dir.path().join("nope")).unwrap_err();
        assert!(matches!(err, VectorError::IndexNotFound { .. }));
    }

    #[test]
    fn dimension_mismatch_is_typed_error() {
        let dir = tempfile::tempdir().unwrap();
        let index = VectorIndex::create(dir.path(), test_config(8)).unwrap();
        let err = index
            .insert(VectorId(1), &vec_of(1, 4), Metadata::empty())
            .unwrap_err();
        match err {
            VectorError::InvalidDimension { expected, got } => {
                assert_eq!((expected, got), (8, 4));
            }
            other => panic!("expected InvalidDimension, got {other:?}"),
        }
        let err = index.search(&vec_of(1, 4), 3).unwrap_err();
        assert!(matches!(err, VectorError::InvalidDimension { .. }));
    }

    #[test]
    fn search_validation() {
        let dir = tempfile::tempdir().unwrap();
        let index = VectorIndex::create(dir.path(), test_config(4)).unwrap();
        assert!(matches!(
            index.search(&vec_of(1, 4), 0),
            Err(VectorError::InvalidArgument { .. })
        ));
        // Empty index search returns empty, no error.
        assert!(index.search(&vec_of(1, 4), 5).unwrap().is_empty());
    }

    #[test]
    fn delete_is_respected_across_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let index = VectorIndex::create(dir.path(), test_config(4)).unwrap();
        for i in 0..5u128 {
            index
                .insert(VectorId(i + 1), &vec_of(i as usize, 4), Metadata::empty())
                .unwrap();
        }
        index.flush().unwrap();
        let query = vec_of(1, 4);
        assert!(index
            .search(&query, 5)
            .unwrap()
            .iter()
            .any(|hit| hit.id == VectorId(2)));
        index.delete(VectorId(2)).unwrap();
        assert!(!index
            .search(&query, 5)
            .unwrap()
            .iter()
            .any(|hit| hit.id == VectorId(2)));

        let reopened = VectorIndex::open(dir.path()).unwrap();
        assert!(!reopened
            .search(&query, 5)
            .unwrap()
            .iter()
            .any(|hit| hit.id == VectorId(2)));
    }

    #[test]
    fn filtered_search_finds_only_matching_route() {
        let dir = tempfile::tempdir().unwrap();
        let index = VectorIndex::create(dir.path(), test_config(4)).unwrap();
        let route_a = VectorId(100);
        let route_b = VectorId(200);
        for i in 0..6u128 {
            let metadata = Metadata {
                route_id: Some(if i % 2 == 0 { route_a } else { route_b }),
                ..Metadata::empty()
            };
            index
                .insert(VectorId(i + 1), &vec_of(i as usize, 4), metadata)
                .unwrap();
        }
        let filter = MetadataFilter::new().route_id_eq(route_a);
        let hits = index
            .search_filtered(&vec_of(1, 4), 10, Some(&filter))
            .unwrap();
        assert!(!hits.is_empty());
        assert!(hits
            .iter()
            .all(|hit| hit.metadata.route_id == Some(route_a)));
    }

    #[test]
    fn upsert_replaces_vector() {
        let dir = tempfile::tempdir().unwrap();
        let index = VectorIndex::create(dir.path(), test_config(4)).unwrap();
        index
            .insert(VectorId(1), &vec_of(1, 4), Metadata::empty())
            .unwrap();
        index
            .insert(VectorId(2), &vec_of(2, 4), Metadata::empty())
            .unwrap();
        // Upsert id 2 far away; it must no longer be nearest to seed 2.
        index
            .insert(VectorId(2), &vec_of(3, 4), Metadata::empty())
            .unwrap();
        let hits = index.search(&vec_of(2, 4), 2).unwrap();
        assert_eq!(hits[0].id, VectorId(1));
        assert_eq!(index.stats().unwrap().live_count, 2);
    }

    #[test]
    fn compact_rebuilds_dense_rows() {
        let dir = tempfile::tempdir().unwrap();
        let index = VectorIndex::create(dir.path(), test_config(4)).unwrap();
        for i in 0..20u128 {
            index
                .insert(VectorId(i + 1), &vec_of(i as usize, 4), Metadata::empty())
                .unwrap();
        }
        for i in 0..15u128 {
            index.delete(VectorId(i + 1)).unwrap();
        }
        let stats = index.stats().unwrap();
        assert_eq!(stats.live_count, 5);
        assert_eq!(stats.deleted_count, 15);
        assert!(stats.should_compact);

        index.compact().unwrap();
        let stats = index.stats().unwrap();
        assert_eq!(stats.live_count, 5);
        assert_eq!(stats.deleted_count, 0);
        assert_eq!(stats.row_count, 5);

        let query = vec_of(19, 4);
        let hits = index.search(&query, 5).unwrap();
        assert_eq!(hits.len(), 5);

        // Compact state survives reopen.
        let reopened = VectorIndex::open(dir.path()).unwrap();
        assert_eq!(reopened.stats().unwrap().row_count, 5);
        let reopened_hits = reopened.search(&query, 5).unwrap();
        assert_eq!(hits, reopened_hits);
    }

    #[test]
    fn operations_after_close_error() {
        let dir = tempfile::tempdir().unwrap();
        let index = VectorIndex::create(dir.path(), test_config(4)).unwrap();
        index.close().unwrap();
        assert!(matches!(
            index.insert(VectorId(1), &vec_of(1, 4), Metadata::empty()),
            Err(VectorError::IndexClosed)
        ));
        assert!(matches!(
            index.search(&vec_of(1, 4), 1),
            Err(VectorError::IndexClosed)
        ));
    }

    #[test]
    fn zero_vector_cosine_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let index = VectorIndex::create(dir.path(), test_config(4)).unwrap();
        assert!(matches!(
            index.insert(VectorId(1), &[0.0; 4], Metadata::empty()),
            Err(VectorError::InvalidVector { .. })
        ));
    }

    #[test]
    fn adaptive_switches_to_exact_for_small_indexes() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = test_config(4);
        config.algorithm = Algorithm::Adaptive;
        config.exact_threshold = 100;
        let index = VectorIndex::create(dir.path(), config).unwrap();
        for i in 0..10u128 {
            index
                .insert(VectorId(i + 1), &vec_of(i as usize, 4), Metadata::empty())
                .unwrap();
        }
        // Small index → exact path → results equal ground truth by definition.
        let hits = index.search(&vec_of(2, 4), 10).unwrap();
        assert_eq!(hits.len(), 10);
    }

    #[test]
    fn filter_with_kind() {
        let dir = tempfile::tempdir().unwrap();
        let index = VectorIndex::create(dir.path(), test_config(4)).unwrap();
        index
            .insert(VectorId(1), &vec_of(1, 4), Metadata::with_kind("chunk"))
            .unwrap();
        index
            .insert(VectorId(2), &vec_of(1, 4), Metadata::with_kind("summary"))
            .unwrap();
        let filter = MetadataFilter::new().kind_eq("chunk");
        let hits = index
            .search_filtered(&vec_of(1, 4), 2, Some(&filter))
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, VectorId(1));
    }
}
