//! Verruckt Vector Engine — embedded vector search for the Verruckt desktop backend.
//!
//! Standalone, self-contained research crate: exact + HNSW search over a
//! persistent, crash-safe, checksummed on-disk format, with WAL, tombstone
//! deletes, metadata filtering, and compaction. It does not touch any
//! mainline production code; see README.md for design and docs/ for the
//! on-disk format, HNSW notes, and benchmarks.
//!
//! ```no_run
//! use verruckt_vector::{VectorIndex, IndexConfig, Metric, VectorId, Metadata};
//! use std::path::Path;
//!
//! # fn main() -> verruckt_vector::Result<()> {
//! let config = IndexConfig::new(768, Metric::Cosine);
//! let index = VectorIndex::create(Path::new("./data/vectors").to_owned().as_path(), config)?;
//! index.insert(VectorId(42), &[0.1; 768], Metadata::empty())?;
//! index.flush()?;
//! let hits = index.search(&[0.1; 768], 10)?;
//! # Ok(())
//! # }
//! ```

// Policy: zero unsafe in all engine code. `deny` (not `forbid`) so that
// exactly one audited unsafe call — the mmap in persist::mmap_segment, with
// a full SAFETY argument — can be explicitly opted into; grep the crate for
// "unsafe" and you will find only that one site plus this comment.
#![deny(unsafe_code)]

pub mod config;
pub mod distance;
pub mod engine;
pub mod error;
pub mod id;
pub mod metadata;

mod codec;
mod exact;
mod hnsw;
mod persist;
mod storage;
mod wal;

pub use config::{Algorithm, EmbeddingSpace, HnswParams, IndexConfig, Metric, PersistenceParams};
pub use engine::{EngineOptions, IndexStats, SearchResult, VectorIndex};
pub use error::{Result, VectorError};
pub use id::VectorId;
pub use metadata::{FilterValue, Metadata, MetadataField, MetadataFilter, Predicate};
