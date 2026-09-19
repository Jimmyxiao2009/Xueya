use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use verruckt_vector::{Algorithm, IndexConfig, Metadata, Metric, VectorId, VectorIndex};

fn vector(seed: usize, dim: usize) -> Vec<f32> {
    let mut state = (seed as u64 * 0x9E37_79B9_7F4A_7C15) | 1;
    (0..dim)
        .map(|_| {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            ((state >> 33) as f32 / u32::MAX as f32) * 2.0 - 1.0
        })
        .collect()
}

fn build(dir: &std::path::Path, n: usize, algorithm: Algorithm) -> VectorIndex {
    let mut config = IndexConfig::new(64, Metric::Cosine);
    config.algorithm = algorithm;
    config.hnsw.ef_construction = 80;
    config.hnsw.ef_search = 64;
    let index = VectorIndex::create(dir, config).unwrap();
    for i in 0..n {
        let v = vector(i, 64);
        index
            .insert(VectorId(i as u128 + 1), &v, Metadata::empty())
            .unwrap();
    }
    index
}

fn search_bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("search");
    for n in [1_000usize, 10_000] {
        let exact_dir = tempfile::tempdir().unwrap();
        let hnsw_dir = tempfile::tempdir().unwrap();
        let exact = build(exact_dir.path(), n, Algorithm::Exact);
        let hnsw = build(hnsw_dir.path(), n, Algorithm::Hnsw);
        let query = vector(999_999, 64);
        group.bench_with_input(BenchmarkId::new("exact", n), &n, |b, _| {
            b.iter(|| black_box(exact.search(black_box(&query), 10).unwrap()))
        });
        group.bench_with_input(BenchmarkId::new("hnsw", n), &n, |b, _| {
            b.iter(|| black_box(hnsw.search(black_box(&query), 10).unwrap()))
        });
    }
    group.finish();
}

fn insert_bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert");
    group.sample_size(10);
    group.bench_function("batch_1000_dim64", |b| {
        b.iter(|| {
            let dir = tempfile::tempdir().unwrap();
            let index =
                VectorIndex::create(dir.path(), IndexConfig::new(64, Metric::Cosine)).unwrap();
            for i in 0..1_000usize {
                let v = vector(i, 64);
                index
                    .insert(VectorId(i as u128), &v, Metadata::empty())
                    .unwrap();
            }
            black_box(index.stats().unwrap())
        })
    });
    group.finish();
}

criterion_group!(benches, search_bench, insert_bench);
criterion_main!(benches);
