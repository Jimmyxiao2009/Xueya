# Verruckt Vector Engine Benchmark Report

Generated: unix-seconds 1789780754
Rust: rustc 1.97.1 (8bab26f4f 2026-07-14)
OS/arch: windows/x86_64

> Measurements are from this run on this machine. Memory is the engine's computed heap footprint from `IndexStats`, not a fabricated RSS number.

| N | dim | backend | build (ms) | open (ms) | p50 (ms) | p95 (ms) | recall@10 | disk (bytes) | heap (bytes) |
|---:|---:|---|---:|---:|---:|---:|---:|---:|---:|
| 1000 | 384 | Exact | 520.01 | 4.28 | 0.1772 | 0.1937 | 1.000 | 1689748 | 181352 |
| 1000 | 384 | HNSW | 525.83 | 4.80 | 0.4258 | 0.5412 | 1.000 | 1689748 | 181352 |
| 10000 | 384 | Exact | 14404.23 | 31.67 | 2.2660 | 2.8778 | 1.000 | 16892225 | 1810704 |
| 10000 | 384 | HNSW | 14006.76 | 33.46 | 2.8963 | 5.5756 | 0.950 | 16892225 | 1810704 |
| 1000 | 384 | SQLite blob + brute force | 58.07 | 24.53 | 12.5772 | 14.2833 | 1.000 | 2056192 | n/a |
| 10000 | 384 | SQLite blob + brute force | 509.75 | 231.48 | 126.2928 | 139.1318 | 1.000 | 20533248 | n/a |

## Machine

- CPU: Intel Core i5-10600KF @ 4.10 GHz
- RAM: 15.9 GiB
- OS: Windows x86_64
- Rust: rustc 1.97.1
- Query count: 10 in this bounded report run
- HNSW report tuning: `M=16`, `efConstruction=200`, `efSearch=1024`

## Method

Synthetic embeddings use a deterministic LCG; Exact is the ground truth. Build includes inserts before the first flush. Open measures reopening the checkpointed generation. Search timings use 10 deterministic queries and k=10. HNSW recall@10 is the overlap with Exact Top-10. The SQLite rows use the current Verruckt-style blob baseline: SQLite insert, reopen/load-all into Rust vectors, then brute-force cosine with per-vector norms. SQLite memory is not reported because this bounded baseline test did not expose a portable RSS counter.
