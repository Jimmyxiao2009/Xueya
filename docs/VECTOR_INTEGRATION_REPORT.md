# Desktop Vector Integration Report

## Summary

The standalone Verruckt Vector Engine is now wired into the desktop RAG
pipeline behind the existing `VectorIndex` trait, with SQLite remaining the
production default backend. An experimental AppData-backed adapter
(`VERRUCKT_VECTOR_BACKEND=verruckt`) is fully functional end to end:
Source import → Chunk → Embedding → VerrucktVectorStore → FTS5 → RRF →
Retriever → Tutor → Citation.

## Branch / HEAD

- Branch: `feature/vector-desktop-integration`
- Base: `v0.1.0-rc.1-competition` plus the two standalone engine commits
  (`82f09e7`, `45b76bd`) cherry-picked, plus `fix(vector): restore stable id
  module on integration base` (the competition tag predates the scaffold's
  `id.rs`).
- Default backend: **SQLite** (`KEEP SQLITE DEFAULT`). The Verruckt backend is
  opt-in via `VERRUCKT_VECTOR_BACKEND=verruckt` or the test-only constructor
  `AppState::new_with_secrets_and_backend`.

## Adapter

`src-tauri/src/knowledge/verruckt_vector.rs` implements the production
`VectorIndex` trait. Key properties:

- **SQLite stays authoritative.** Adapter `upsert` writes the `embeddings`
  row first (preserving the existing cache/lifecycle contract with zero extra
  embedding-provider calls), then mirrors the vector into the derived index.
- **One Exact index per embedding space** `(provider, model, dimension)`;
  directories live at `<app_data_dir>/vector/<sha256-identity>/`, derived from
  provider UUID + model + dimension + format id. Chunk UUIDs are the external
  `VectorId`s; dense row ids stay internal.
- **Algorithm:** Exact is intentional for the first integration. Benchmarks
  (below) show no usable HNSW crossover at desktop scale under the recall
  constraint, and the production trait's `search_filtered` accepts an
  arbitrary chunk-ID set which Exact serves with full route/evidence
  correctness.
- **Lifecycle guard:** rebuild/flush take a write guard; searches and upserts
  take a read guard, so derived directories are never deleted while an
  operation owns an engine handle.
- **Corruption recovery:** a corrupt/unsupported derived index is detected at
  open and rebuilt from SQLite embeddings without crashing the app and
  without re-embedding. If a startup rebuild fails, all
  `source_index_state` rows are marked `needs_reindex`.
- **Delete semantics:** adapter `delete` tombstones every authoritative space
  and never blocks the authoritative source deletion; the explicit source
  delete path (see lifecycle) removes SQLite rows first.

## Index lifecycle

```
Source parse:  Importing → Ready            (source.status; parse availability)
Index state:   indexing → ready             (source_index_state; checkpointed)
               indexing → failed            (upsert/flush error)
               needs_reindex / interrupted  (provider change / crash recovery)
```

- `source_index_state == ready` is only recorded **after** every vector upsert
  **and** one successful `VectorIndex::flush()` (full engine checkpoint).
  `Ready` is never recorded before the checkpoint.
- Source parse status (`sources.status`) is separate from index status.
- Source deletion is now explicit and transactional: index state → embeddings
  → chunks → documents → revisions → source row. This also fixes a real RC
  defect: the FTS5 delete trigger used the external-content delete command and
  failed with `SQLITE_ERROR` during source cascades
  (`migrations/0006_fix_fts_delete.sql`).
- Reindex uses existing cached embeddings; a provider/model/dimension change
  still marks old states `needs_reindex` and never mixes embedding spaces.

## Tests (Windows, this machine)

- Frontend: `npm run typecheck`, `npm run lint`, `npm run test` (30 tests),
  `npm run build` — all green.
- `src-tauri`: 71 unit + 59 integration tests green (12 cloud, 8 provider,
  14 learning, 1 competition demo, 1 golden path, 4 retrieval, 8 vector
  integration, 1 sqlite baseline runner ignored/live ignored), plus
  `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`.
- `crates/verruckt-vector`: 35 unit + 2 doc + 3 concurrency + 2 recall +
  13 persistence/crash + 1 property + 5 RAG fixture + 1 RAG regression +
  1 SQLite baseline — green in debug and release.
- New desktop integration coverage
  (`src-tauri/tests/vector_integration.rs`, 8 tests): ingestion readiness and
  AppData space dirs, hybrid route isolation across reopen, RAG Top1 plus
  tutor citation grounding for “为什么平行线在射影平面中会相交？”, reindex with
  zero new provider calls, corrupt derived index recovered from SQLite on
  reopen, interrupted index not ready until reindexed, SQLite default
  constructor, and source delete removing authoritative + derived vectors.

## Real-corpus RAG regression

30 real-text queries (Projective Geometry + Linear Algebra), deterministic
embeddings, dim 256 (`cargo test --test rag_regression -- --nocapture`):

| backend | Top1 | Recall@5 | Recall@10 |
|---|---|---|---|
| SQLite blob + brute force | 0.867 | 1.000 | 1.000 |
| Verruckt Exact | 0.867 | 1.000 | 1.000 |
| Verruckt HNSW | 0.867 | 1.000 | 1.000 |

Backend switching does not change retrieval quality; candidate ordering
entropy is identical across the three backends.

## Crossover benchmark (1k–100k, dim 384, 50 queries, deterministic LCG)

Machine: Intel Core i5-10600KF @ 4.10 GHz, 15.9 GiB RAM, Windows x86_64,
rustc 1.97.1. Full table in `crates/verruckt-vector/docs/CROSSOVER.md`.

| N | Exact p50 | HNSW p50 | HNSW recall@10 |
|---:|---:|---:|---:|
| 1,000 | 0.18 ms | 0.31 ms | 1.000 |
| 10,000 | 2.17 ms | 2.68 ms | 0.904 |
| 50,000 | 11.13 ms | 4.88 ms | 0.518 |
| 100,000 | 22.15 ms | 6.20 ms | 0.334 |

- **Crossover:** the raw latency crossover sits between 10k and 50k.
- **Usable crossover:** not observed. At ≥50k HNSW is faster but its
  recall@10 collapses below 0.55 with the current tuning on uniform random
  vectors, and at 10k it does not reach 0.95 either. Under the
  Recall@10 ≥ 0.95 constraint, **no crossover observed in tested range** —
  Exact remains the correct desktop default.

### Why the two earlier numbers differ (micro vs end-to-end)

- Criterion **microbenchmark** measures the core search kernel only
  (10k: Exact ≈ 354 µs, HNSW ≈ 80 µs).
- The **end-to-end report** measures validation, query normalization, filter
  application, result materialization and locking (10k: Exact ≈ 2.2 ms,
  HNSW ≈ 2.7 ms).
- The **SQLite blob baseline** additionally includes BLOB retrieval and
  materialization plus Rust cosine with per-vector norms (10k p50 ≈ 126 ms).
  The correct claim is therefore: “Verruckt Vector Engine's dedicated storage
  layout and retrieval path significantly reduce query overhead compared with
  the SQLite blob vector backend”, not “our algorithm is 50x faster”.

## Cost

Switching backends triggers **no additional embedding-provider calls** while
the embedding cache is valid: the adapter rebuilds derived indexes from
SQLite's cached vectors and `embedding_cache` rows. Verified by
`reindex_reuses_embedding_cache_without_new_provider_calls`.

## Dev diagnostics

`vector_backend_info` (backend-only, registered in the frontend contract as
`BACKEND_ONLY`) exposes backend, algorithm, vector count, deleted count,
dimension and index size through the trait's `info()` seam.

## Limitations

- The Verruckt backend is opt-in; ordinary users never see it and the
  production default is unchanged.
- Shadow mode was deliberately deferred: the basic adapter is stable, but a
  double-write path is not yet justified by the benchmark evidence.
- HNSW at ≥50k needs retuned parameters (higher `M`/`efConstruction`, corpus-
  dependent `efSearch`) before its latency advantage is usable; on cluster-
  structured real corpora recall will differ from uniform random vectors.
- Desktop app smoke was validated through the full Rust integration suite
  (which exercises the real ingestion → retrieval → tutor → citation services
  against temp AppData profiles); a manual `npm run tauri dev` session was not
  performed in this environment.

## Recommendation

**KEEP SQLITE DEFAULT.** The adapter is production-ready as an opt-in
experimental backend, but the benchmark evidence does not yet justify making
it the default: Exact already covers desktop scale (≤100k) at 2–22 ms p50 with
recall 1.0, and HNSW's only wins come with unacceptable recall under the
current tuning.
