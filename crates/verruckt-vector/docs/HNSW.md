# HNSW notes

## Source

The implementation follows the core algorithms in:

> Yu. A. Malkov and D. A. Yashunin, “Efficient and robust approximate nearest neighbor search using Hierarchical Navigable Small World graphs,” IEEE TPAMI, 2018.

It also follows the terminology used by the public hnswlib implementation for `M`, `efConstruction`, `ef`, upper-layer greedy descent, and the heuristic neighbor-selection rule.

## Implementation

- Layer assignment is sampled from the standard exponentially decaying distribution, with a deterministic ChaCha8 stream seeded from `(IndexConfig.hnsw.seed, wal_seq)`.
- Layer 0 has `2M` link slots; upper layers have `M` slots.
- Link storage is one flat `Vec<u32>` with per-node offsets and sentinel-filled fixed-capacity blocks. This avoids a `Vec<Vec<u32>>` for graph hot paths and makes graph serialization explicit.
- Construction uses `ef_construction` candidate search and heuristic neighbor selection with keep-pruned connections.
- Search performs greedy descent with `ef=1` through upper layers and an `ef_search` beam on layer 0.
- Deleted nodes remain traversable and are excluded only from result sets; this preserves graph connectivity until compaction.
- Distance kernels use the engine metric: normalized dot for cosine, negative dot for Dot, and squared L2 distance for graph traversal.

## Differences and deliberate simplifications

1. The engine uses a fixed-capacity flat slot layout rather than the variable-length adjacency vectors common in reference implementations. The observable graph behavior is the same; empty slots are explicit sentinels.
2. Filtered search applies metadata predicates at result admission, not during graph topology construction. It progressively expands `ef` and falls back to exact filtered scan for selective predicates. This favors correctness for Verruckt's 1k–100k local corpus over a complex filtered bitmap index.
3. The engine uses deterministic per-sequence level assignment. Production can choose a different seed in `IndexConfig`; tests use a fixed seed so recall and persistence are reproducible.
4. There is no background maintenance thread. `should_compact()` and `compact()` leave scheduling to the desktop caller.

## Recall target

Default tuning targets Recall@10 ≥ 0.95. Exact search is retained permanently as ground truth, adaptive fallback, and a filter fallback. The integration suite contains deterministic recall@10 and fixed-seed determinism tests; the release benchmark report provides the workload-specific numbers used for an integration decision.
