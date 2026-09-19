//! Model-based property test: random op sequences (insert / delete / search /
//! flush / reopen) applied to both the engine and a naive HashMap reference;
//! every search must agree with the reference's exact top-k.

use proptest::prelude::*;
use verruckt_vector::{IndexConfig, Metadata, Metric, VectorId, VectorIndex};

/// Small dimension keeps each case fast; LCG keeps vectors deterministic.
fn lcg_vec(seed: u64, dim: usize) -> Vec<f32> {
    let mut state = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    (0..dim)
        .map(|_| {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((state >> 33) as f32 / u32::MAX as f32) * 2.0 - 1.0
        })
        .collect()
}

#[derive(Debug, Clone)]
enum Op {
    Insert { key: u16, seed: u64 },
    Upsert { key: u16, seed: u64 },
    Delete { key: u16 },
    Search { query_seed: u64, k: u8 },
    Flush,
    Reopen,
}

fn op_strategy() -> impl Strategy<Value = Op> {
    prop_oneof![
        (any::<u16>(), any::<u64>()).prop_map(|(key, seed)| Op::Insert { key, seed }),
        (any::<u16>(), any::<u64>()).prop_map(|(key, seed)| Op::Upsert { key, seed }),
        any::<u16>().prop_map(|key| Op::Delete { key }),
        (any::<u64>(), 1u8..=8).prop_map(|(query_seed, k)| Op::Search { query_seed, k }),
        Just(Op::Flush),
        Just(Op::Reopen),
    ]
}

/// Reference model: HashMap of live rows; exact cosine top-k by brute force.
struct Model {
    dim: usize,
    rows: std::collections::HashMap<u16, Vec<f32>>,
}

impl Model {
    fn top_k(&self, query: &[f32], k: usize) -> Vec<u16> {
        let mut scored: Vec<(f32, u16)> = self
            .rows
            .iter()
            .map(|(key, vector)| (cosine(query, vector), *key))
            .collect();
        scored.sort_by(|a, b| {
            b.0.partial_cmp(&a.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.1.cmp(&b.1))
        });
        scored.truncate(k);
        scored.into_iter().map(|(_, key)| key).collect()
    }
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na <= f32::EPSILON || nb <= f32::EPSILON {
        return 0.0;
    }
    dot / (na * nb)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    #[test]
    fn random_op_sequences_match_reference(
        ops in prop::collection::vec(op_strategy(), 8..48),
    ) {
        let dir = tempfile::tempdir().unwrap();
        let mut config = IndexConfig::new(12, Metric::Cosine);
        config.exact_threshold = 4; // exercise the exact path in these sizes
        let mut index = VectorIndex::create(dir.path(), config).unwrap();
        let mut model = Model { dim: 12, rows: std::collections::HashMap::new() };

        for op in ops {
            match op {
                Op::Insert { key, seed } => {
                    let vector = lcg_vec(seed | 1, model.dim);
                    // Normalize like the engine does for cosine.
                    let norm: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
                    if norm > f32::EPSILON {
                        let normalized: Vec<f32> = vector.iter().map(|x| x / norm).collect();
                        if index.insert(VectorId(key as u128), &normalized, Metadata::empty()).is_ok() {
                            model.rows.insert(key, normalized);
                        }
                    }
                }
                Op::Upsert { key, seed } => {
                    let vector = lcg_vec(seed | 1, model.dim);
                    let norm: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
                    if norm > f32::EPSILON {
                        let normalized: Vec<f32> = vector.iter().map(|x| x / norm).collect();
                        index.insert(VectorId(key as u128), &normalized, Metadata::empty()).unwrap();
                        model.rows.insert(key, normalized);
                    }
                }
                Op::Delete { key } => {
                    index.delete(VectorId(key as u128)).unwrap();
                    model.rows.remove(&key);
                }
                Op::Search { query_seed, k } => {
                    let query_raw = lcg_vec(query_seed | 1, model.dim);
                    let norm: f32 = query_raw.iter().map(|x| x * x).sum::<f32>().sqrt();
                    if norm <= f32::EPSILON {
                        continue; // engine rejects zero queries for cosine
                    }
                    let query: Vec<f32> = query_raw.iter().map(|x| x / norm).collect();
                    let expected = model.top_k(&query, k as usize);
                    let hits = index.search(&query, k as usize).unwrap();
                    let got: Vec<u16> = hits.iter().map(|h| h.id.0 as u16).collect();
                    // IDs must match (scores may differ in the last ulps).
                    prop_assert_eq!(got, expected);
                }
                Op::Flush => {
                    index.flush().unwrap();
                }
                Op::Reopen => {
                    index.flush().unwrap();
                    drop(index);
                    index = VectorIndex::open(dir.path()).unwrap();
                    // Model survives reopen unchanged (that is the point).
                }
            }
        }
    }
}
