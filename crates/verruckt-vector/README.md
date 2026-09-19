# Verruckt Vector Engine

Standalone embedded vector engine for Verruckt Desktop. This crate is intentionally isolated under `crates/verruckt-vector/` and does not provide a server, REST API, cloud service, account system, or distributed database.

## Design

- **Embedded:** synchronous Rust API, suitable for the desktop Rust backend.
- **Predictable scale:** designed for roughly 1,000–100,000 chunks, not billion-vector SaaS workloads.
- **Exact + HNSW:** Exact is the ground truth and fallback; HNSW is the approximate path.
- **Flat vectors:** contiguous row-major `f32` storage, friendly to compiler auto-vectorization.
- **Persistent:** generation-based segment files, atomic manifest commit, little-endian framing, FNV-1a corruption detection.
- **Crash-safe:** mutations are appended to a WAL before memory state changes. A torn WAL tail is truncated during recovery; committed generations remain readable after a checkpoint crash.
- **Metadata:** typed route/source/knowledge-point/kind fields with conjunction and equality/set filters.
- **Deletes:** tombstones; `compact()` rebuilds dense rows and a fresh graph.
- **Concurrency:** many concurrent searches through `RwLock`; one serialized writer; searches see a consistent snapshot.

## Public API

```rust
let index = VectorIndex::create(path, IndexConfig::new(768, Metric::Cosine))?;
index.insert(id, &embedding, Metadata::empty())?;
index.insert_batch(items)?;
let hits = index.search(&query, 10)?;
let filtered = index.search_filtered(&query, 10, Some(&filter))?;
index.flush()?;       // full durable checkpoint
index.compact()?;     // remove tombstones
let stats = index.stats()?;
index.close()?;
```

Scores always mean **higher is more similar**:

- Cosine: `[-1, 1]`; vectors are normalized on insert.
- Dot: raw inner product.
- L2: negative Euclidean distance (`0` is identical).

A dimension is fixed at creation. Wrong dimensions, non-finite values, zero cosine vectors, invalid `k`, corrupt files, and unsupported versions return typed errors; malformed input never panics.

## Architecture

See [ARCHITECTURE.md](ARCHITECTURE.md). The core components are `VectorIndex`, `Rows`, `ExactSearcher`, `HnswGraph`, `Manifest`, segment codecs, and `WalWriter`.

## HNSW

This is an independent implementation, not a wrapper around an HNSW/vector database crate. It uses multi-layer graph search, deterministic ChaCha8 level assignment, configurable `M`, `ef_construction`, `ef_search`, and heuristic neighbor selection. See [docs/HNSW.md](docs/HNSW.md).

## Persistence, WAL, and recovery

Checkpoint segments are generation-suffixed (`vectors.<generation>.dat`, etc.). The manifest is the commit point. A checkpoint writes and fsyncs a complete new generation, atomically renames `manifest.bin`, and then rotates the WAL. If the process dies before the manifest rename, the old manifest and old generation remain valid. If it dies after the manifest rename but before WAL rotation, WAL records at or below the manifest sequence are skipped during replay.

`flush()` performs a full checkpoint. Inserts are WAL-appended without a per-operation `FlushFileBuffers` call by default; call `flush()` for a durability boundary. The engine never silently accepts a bad manifest or segment checksum.

## Metadata filtering

The first implementation intentionally avoids an inverted bitmap database. HNSW performs candidate search, applies the metadata predicate, expands `ef` when too few filtered hits survive, and finally falls back to exact filtered search when necessary. Small adaptive indexes use exact search directly.

## Benchmarking

Run the focused Criterion suite:

```text
cargo bench
```

Generate a Markdown matrix from real measurements:

```text
cargo run --release --bin verruckt-vector-report -- --sizes 1000,10000 --dims 384,768 --queries 100
```

The report includes build/open time, p50/p95 search latency, Exact-vs-HNSW recall@10, disk bytes, and computed engine heap bytes. It does not invent RSS or benchmark values. See [docs/BENCHMARKS.md](docs/BENCHMARKS.md).

## Safety and portability

The crate has a zero-unsafe policy for engine logic. One audited exception is the `memmap2::Mmap::map` call used for the read-only checkpoint vector prefix; its SAFETY argument is inline in `persist.rs`. The file format is explicitly little-endian and does not dump Rust struct memory. A portable heap mode remains available through `EngineOptions`.

## Limitations

- One process should own an index directory at a time; no distributed locking or server mode is provided.
- `compact()` is synchronous and caller-triggered; background compaction is intentionally out of scope.
- Metadata is deliberately small and typed, not a JSON document database.
- The current SQLite comparison is a benchmark baseline only; the production SQLite backend remains the default until an adapter and integration gate pass.
- Mmap generation cleanup is conservative on platforms that keep a mapped file open; stale generations are harmless and cleaned on a later open/checkpoint.
