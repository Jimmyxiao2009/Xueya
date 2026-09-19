//! HNSW recall tests against the Exact ground truth.

use verruckt_vector::{Algorithm, IndexConfig, Metadata, Metric, VectorId, VectorIndex};

fn lcg(seed: u64, dim: usize) -> Vec<f32> {
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

fn clustered(seed: u64, dim: usize, cluster: usize) -> Vec<f32> {
    let mut v = lcg(seed * 7919 + cluster as u64, dim);
    // Strong cluster axis makes the synthetic corpus stable and meaningful.
    v[cluster % dim] += 4.0;
    v
}

#[test]
fn hnsw_recall_at_10_exceeds_095_on_deterministic_corpus() {
    let dir_h = tempfile::tempdir().unwrap();
    let dir_e = tempfile::tempdir().unwrap();
    let mut hcfg = IndexConfig::new(32, Metric::Cosine);
    hcfg.algorithm = Algorithm::Hnsw;
    hcfg.hnsw.m = 16;
    hcfg.hnsw.ef_construction = 100;
    hcfg.hnsw.ef_search = 96;
    hcfg.hnsw.seed = 0x5EED;
    let mut ecfg = hcfg.clone();
    ecfg.algorithm = Algorithm::Exact;

    let hnsw = VectorIndex::create(dir_h.path(), hcfg).unwrap();
    let exact = VectorIndex::create(dir_e.path(), ecfg).unwrap();
    let n = 1200usize;
    for i in 0..n {
        let vector = clustered(i as u64 + 1, 32, i % 32);
        let id = VectorId(i as u128 + 1);
        hnsw.insert(id, &vector, Metadata::empty()).unwrap();
        exact.insert(id, &vector, Metadata::empty()).unwrap();
    }

    let mut total = 0usize;
    let mut recovered = 0usize;
    for q in 0..100usize {
        let query = clustered(10_000 + q as u64, 32, q % 32);
        let truth: std::collections::HashSet<VectorId> = exact
            .search(&query, 10)
            .unwrap()
            .into_iter()
            .map(|hit| hit.id)
            .collect();
        let approx = hnsw.search(&query, 10).unwrap();
        recovered += approx.iter().filter(|hit| truth.contains(&hit.id)).count();
        total += truth.len();
    }
    let recall = recovered as f64 / total as f64;
    assert!(
        recall >= 0.95,
        "HNSW recall@10 was {recall:.3}, expected >= 0.95"
    );
}

#[test]
fn hnsw_results_are_deterministic_with_fixed_seed() {
    let mut cfg = IndexConfig::new(16, Metric::Cosine);
    cfg.algorithm = Algorithm::Hnsw;
    cfg.hnsw.seed = 12345;
    cfg.hnsw.ef_construction = 80;
    cfg.hnsw.ef_search = 64;
    let dir_a = tempfile::tempdir().unwrap();
    let dir_b = tempfile::tempdir().unwrap();
    let a = VectorIndex::create(dir_a.path(), cfg.clone()).unwrap();
    let b = VectorIndex::create(dir_b.path(), cfg).unwrap();
    for i in 0..300u128 {
        let v = lcg(i as u64 + 77, 16);
        a.insert(VectorId(i + 1), &v, Metadata::empty()).unwrap();
        b.insert(VectorId(i + 1), &v, Metadata::empty()).unwrap();
    }
    let q = lcg(9999, 16);
    assert_eq!(a.search(&q, 10).unwrap(), b.search(&q, 10).unwrap());
}
