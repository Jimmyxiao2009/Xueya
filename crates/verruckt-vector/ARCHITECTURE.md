# Architecture

```text
VectorIndex
├── RwLock<Inner>
│   ├── Rows
│   │   ├── VectorData
│   │   │   ├── Heap: contiguous Vec<f32>
│   │   │   └── Mmap: checkpoint prefix + mutable tail
│   │   ├── external ids: Vec<VectorId>
│   │   ├── idmap: HashMap<VectorId, dense row>
│   │   ├── Metadata rows
│   │   └── alive bits / tombstones
│   ├── HnswGraph
│   │   ├── deterministic level assignment
│   │   ├── flat fixed-capacity link slots
│   │   ├── greedy upper-layer descent
│   │   └── ef-bounded layer-0 search
│   ├── WalWriter
│   ├── generation / max sequence
│   └── IndexConfig
└── persistence
    ├── Manifest
    ├── vectors.<gen>.dat
    ├── ids.<gen>.dat
    ├── meta.<gen>.dat
    ├── graph.<gen>.dat
    └── wal.log
```

## Search path

1. Validate query dimensionality and finiteness.
2. Normalize a cosine query once.
3. Select Exact or HNSW from `Algorithm` (`Adaptive` uses `exact_threshold`).
4. Exact scans the contiguous vector arena and maintains a bounded top-k heap.
5. HNSW descends greedily through upper layers and performs an `ef` search on layer 0.
6. Filtered HNSW retains navigability through filtered/tombstoned nodes, expands `ef`, and falls back to exact filtered search if necessary.
7. Materialize `SearchResult { id, score, metadata }`.

## Mutation path

1. Validate and normalize the vector.
2. Append a complete WAL record with the next sequence number.
3. Apply the operation to rows and idmap.
4. Insert the new dense row into HNSW (unless the configured algorithm is Exact).
5. Auto-checkpoint if the WAL exceeds its configured threshold.

Upsert is append-only: the old live row becomes a tombstone and the new vector gets a dense row. This keeps mmap-backed checkpoint vectors immutable.

## Checkpoint path

1. Allocate `generation + 1` segment names.
2. Stream vectors, encode ids/metadata/graph, fsync each staged file.
3. Build a manifest containing configuration, counts, sequence, generation, and frame checksums.
4. Atomically rename `manifest.bin.tmp` over `manifest.bin`.
5. Rotate the WAL and garbage-collect stale generations.

The manifest is the only commit point. A crash before step 4 leaves the previous generation selected; a crash after step 4 leaves a complete new generation selected.

## Snapshot semantics

Search holds the read lock for the complete traversal. A concurrent writer waits, and a search that acquires the lock before a writer sees the older snapshot. There is no partial graph update visible to a reader.

## Compaction

Compaction copies live rows in stable row order, rebuilds the idmap and HNSW graph, swaps the in-memory state, and checkpoints it as a new generation. Tombstones are removed; external IDs and metadata remain unchanged.
