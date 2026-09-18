use crate::error::{Result, VectorError};

/// Distance metric for the index.
///
/// Score semantics are uniform across the public API: **higher score means
/// more similar**.
///
/// * `Cosine` — cosine similarity in `[-1, 1]`. Vectors are normalized on
///   insert, so the engine computes a plain dot product at query time.
/// * `Dot` — raw inner product (unbounded).
/// * `L2` — score is `-euclidean_distance` (i.e. `0.0` means identical,
///   more negative means farther). The API never returns raw distances.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Metric {
    Cosine,
    Dot,
    L2,
}

impl Metric {
    /// Stable on-disk discriminant (see docs/FORMAT.md).
    pub(crate) const fn code(self) -> u8 {
        match self {
            Metric::Cosine => 0,
            Metric::Dot => 1,
            Metric::L2 => 2,
        }
    }

    pub(crate) fn from_code(code: u8) -> Result<Metric> {
        match code {
            0 => Ok(Metric::Cosine),
            1 => Ok(Metric::Dot),
            2 => Ok(Metric::L2),
            other => Err(VectorError::corrupt(
                "manifest",
                format!("unknown metric code {other}"),
            )),
        }
    }

    pub(crate) const fn name(self) -> &'static str {
        match self {
            Metric::Cosine => "cosine",
            Metric::Dot => "dot",
            Metric::L2 => "l2",
        }
    }
}

/// Search algorithm selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Algorithm {
    /// Exact brute-force scan only.
    Exact,
    /// HNSW approximate search only.
    Hnsw,
    /// Exact below `exact_threshold` live vectors, HNSW above.
    /// The threshold is an engine tuning parameter, benchmarked in
    /// docs/BENCHMARKS.md.
    Adaptive,
}

impl Algorithm {
    pub(crate) const fn code(self) -> u8 {
        match self {
            Algorithm::Exact => 0,
            Algorithm::Hnsw => 1,
            Algorithm::Adaptive => 2,
        }
    }

    pub(crate) fn from_code(code: u8) -> Result<Algorithm> {
        match code {
            0 => Ok(Algorithm::Exact),
            1 => Ok(Algorithm::Hnsw),
            2 => Ok(Algorithm::Adaptive),
            other => Err(VectorError::corrupt(
                "manifest",
                format!("unknown algorithm code {other}"),
            )),
        }
    }

    pub(crate) const fn name(self) -> &'static str {
        match self {
            Algorithm::Exact => "exact",
            Algorithm::Hnsw => "hnsw",
            Algorithm::Adaptive => "adaptive",
        }
    }
}

/// HNSW construction / search parameters.
///
/// These are engine tuning knobs, not user-facing settings; Verruckt users
/// should always use [`HnswParams::default`].
#[derive(Debug, Clone, PartialEq)]
pub struct HnswParams {
    /// Max connections per node on upper layers (`M` in the HNSW paper).
    pub m: u32,
    /// Size of the candidate list during graph construction.
    pub ef_construction: u32,
    /// Size of the candidate list during search (`ef` must be >= k).
    pub ef_search: u32,
    /// Use the heuristic neighbor selection (Algorithm 4 of the paper)
    /// instead of plain top-M.
    pub heuristic: bool,
    /// RNG seed for level assignment. Fixed seeds keep CI deterministic;
    /// production callers may pass a random seed.
    pub seed: u64,
}

impl Default for HnswParams {
    fn default() -> Self {
        HnswParams {
            m: 16,
            ef_construction: 200,
            ef_search: 128,
            heuristic: true,
            seed: 0x5EED_0000_0000_0001,
        }
    }
}

/// Identity of the embedding space this index belongs to (requirement: one
/// index per embedding space). If provider/model/dimension/source changes the
/// index must be rebuilt, never mixed.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EmbeddingSpace {
    /// Embedding provider id (free-form, e.g. `openai`), ≤ 64 bytes.
    pub provider: String,
    /// Embedding model name, ≤ 128 bytes.
    pub model: String,
    /// Hash of the derived-data source (e.g. chunk corpus hash), ≤ 128 bytes.
    pub source_hash: String,
}

/// Tuning for the mutable tail / checkpoint machinery.
#[derive(Debug, Clone, PartialEq)]
pub struct PersistenceParams {
    /// Checkpoint automatically once the WAL grows past this many bytes.
    pub wal_auto_checkpoint_bytes: u64,
    /// Suggest compaction once deleted vectors exceed this fraction of all
    /// rows (live + deleted).
    pub compact_ratio_threshold: f64,
}

impl Default for PersistenceParams {
    fn default() -> Self {
        PersistenceParams {
            wal_auto_checkpoint_bytes: 64 * 1024 * 1024,
            compact_ratio_threshold: 0.25,
        }
    }
}

/// Index creation configuration. Immutable after `create`.
#[derive(Debug, Clone)]
pub struct IndexConfig {
    /// Fixed vector dimensionality.
    pub dimensions: u32,
    /// Distance metric.
    pub metric: Metric,
    /// Search algorithm.
    pub algorithm: Algorithm,
    /// HNSW tuning.
    pub hnsw: HnswParams,
    /// Persistence tuning.
    pub persistence: PersistenceParams,
    /// Live-vector count below which adaptive search falls back to exact.
    pub exact_threshold: u64,
    /// Embedding space identity recorded in the manifest.
    pub embedding_space: EmbeddingSpace,
}

impl IndexConfig {
    /// Defaults for a given dimensionality and metric.
    pub fn new(dimensions: u32, metric: Metric) -> Self {
        IndexConfig {
            dimensions,
            metric,
            algorithm: Algorithm::Adaptive,
            hnsw: HnswParams::default(),
            persistence: PersistenceParams::default(),
            exact_threshold: 4096,
            embedding_space: EmbeddingSpace::default(),
        }
    }

    pub(crate) fn validate(&self) -> Result<()> {
        if self.dimensions == 0 || self.dimensions > 65_536 {
            return Err(VectorError::invalid_argument(format!(
                "dimensions must be in 1..=65536, got {}",
                self.dimensions
            )));
        }
        if self.hnsw.m == 0 || self.hnsw.m > 512 {
            return Err(VectorError::invalid_argument(format!(
                "hnsw.m must be in 1..=512, got {}",
                self.hnsw.m
            )));
        }
        if self.hnsw.ef_construction == 0 || self.hnsw.ef_construction > 4096 {
            return Err(VectorError::invalid_argument(format!(
                "hnsw.ef_construction must be in 1..=4096, got {}",
                self.hnsw.ef_construction
            )));
        }
        if self.hnsw.ef_search == 0 || self.hnsw.ef_search > 65_536 {
            return Err(VectorError::invalid_argument(format!(
                "hnsw.ef_search must be in 1..=65536, got {}",
                self.hnsw.ef_search
            )));
        }
        if self.provider_len() > 64 {
            return Err(VectorError::invalid_metadata(format!(
                "embedding_space.provider must be <= 64 bytes, got {}",
                self.provider_len()
            )));
        }
        if self.embedding_space.model.len() > 128 {
            return Err(VectorError::invalid_metadata(format!(
                "embedding_space.model must be <= 128 bytes, got {}",
                self.embedding_space.model.len()
            )));
        }
        if self.embedding_space.source_hash.len() > 128 {
            return Err(VectorError::invalid_metadata(format!(
                "embedding_space.source_hash must be <= 128 bytes, got {}",
                self.embedding_space.source_hash.len()
            )));
        }
        Ok(())
    }

    fn provider_len(&self) -> usize {
        self.embedding_space.provider.len()
    }
}
