# Verruckt Vector Engine Benchmark Report

Generated: unix-seconds 1789827235
Rust: rustc 1.97.1 (8bab26f4f 2026-07-14)
OS/arch: windows/x86_64

> Measurements are from this run on this machine. Memory is the engine's computed heap footprint from `IndexStats`, not a fabricated RSS number.

| N | dim | backend | build (ms) | open (ms) | p50 (ms) | p95 (ms) | recall@10 | disk (bytes) | heap (bytes) |
|---:|---:|---|---:|---:|---:|---:|---:|---:|---:|
| 1000 | 384 | Exact | 523.46 | 4.11 | 0.1769 | 0.2034 | 1.000 | 1689748 | 181352 |
| 1000 | 384 | HNSW | 508.78 | 4.89 | 0.3067 | 0.4039 | 1.000 | 1689748 | 181352 |
| 10000 | 384 | Exact | 15277.95 | 29.25 | 2.1739 | 3.4976 | 1.000 | 16892225 | 1810704 |
| 10000 | 384 | HNSW | 14359.18 | 30.25 | 2.6783 | 3.4621 | 0.904 | 16892225 | 1810704 |
| 50000 | 384 | Exact | 114617.88 | 163.42 | 11.1307 | 15.0835 | 1.000 | 84469257 | 9062736 |
| 50000 | 384 | HNSW | 108880.03 | 150.84 | 4.8764 | 5.7981 | 0.518 | 84469257 | 9062736 |
| 100000 | 384 | Exact | 241922.46 | 318.52 | 22.1539 | 29.6169 | 1.000 | 168940995 | 18128224 |
| 100000 | 384 | HNSW | 239986.15 | 345.49 | 6.2014 | 10.2221 | 0.334 | 168940995 | 18128224 |

## Method

Synthetic embeddings use a deterministic LCG; Exact is the ground truth. Build includes inserts before the first flush. Open measures reopening the checkpointed generation. Search timings use 50 deterministic queries and k=10. HNSW recall@10 is the overlap with Exact Top-10.
