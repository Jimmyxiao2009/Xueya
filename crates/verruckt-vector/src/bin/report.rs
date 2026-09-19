//! Reproducible benchmark report generator.
//!
//! Usage:
//!   cargo run --release --bin verruckt-vector-report -- --sizes 1000,10000
//!
//! The default matrix is intentionally bounded for CI; pass larger sizes and
//! dimensions for the release report. Output is Markdown with real wall-clock
//! measurements, never invented values.

use std::path::PathBuf;
use std::time::{Duration, Instant};
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

fn percentile(mut values: Vec<Duration>, p: f64) -> Duration {
    values.sort_unstable();
    let index = ((values.len().saturating_sub(1)) as f64 * p).round() as usize;
    values[index]
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let sizes = arg_list(&args, "--sizes").unwrap_or_else(|| vec![1_000, 10_000]);
    let dims = arg_list(&args, "--dims").unwrap_or_else(|| vec![384]);
    let queries = arg_value(&args, "--queries").unwrap_or(100);

    println!("# Verruckt Vector Engine Benchmark Report\n");
    println!("Generated: {}", chrono_like_now());
    println!("Rust: {}", rustc_version());
    println!(
        "OS/arch: {}/{}\n",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    println!("> Measurements are from this run on this machine. Memory is the engine's computed heap footprint from `IndexStats`, not a fabricated RSS number.\n");
    println!("| N | dim | backend | build (ms) | open (ms) | p50 (ms) | p95 (ms) | recall@10 | disk (bytes) | heap (bytes) |\n|---:|---:|---|---:|---:|---:|---:|---:|---:|---:|");

    for &dim in &dims {
        for &n in &sizes {
            let exact = measure(dim, n, Algorithm::Exact, queries);
            let hnsw = measure(dim, n, Algorithm::Hnsw, queries);
            println!(
                "| {n} | {dim} | Exact | {:.2} | {:.2} | {:.4} | {:.4} | 1.000 | {} | {} |",
                exact.build_ms, exact.open_ms, exact.p50_ms, exact.p95_ms, exact.disk, exact.heap
            );
            println!(
                "| {n} | {dim} | HNSW | {:.2} | {:.2} | {:.4} | {:.4} | {:.3} | {} | {} |",
                hnsw.build_ms,
                hnsw.open_ms,
                hnsw.p50_ms,
                hnsw.p95_ms,
                hnsw.recall,
                hnsw.disk,
                hnsw.heap
            );
        }
    }

    println!("\n## Method\n");
    println!("Synthetic embeddings use a deterministic LCG; Exact is the ground truth. Build includes inserts before the first flush. Open measures reopening the checkpointed generation. Search timings use {queries} deterministic queries and k=10. HNSW recall@10 is the overlap with Exact Top-10.");
}

struct Measurement {
    build_ms: f64,
    open_ms: f64,
    p50_ms: f64,
    p95_ms: f64,
    recall: f64,
    disk: u64,
    heap: u64,
}

fn temp_dir(label: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let path = std::env::temp_dir().join(format!(
        "verruckt-vector-{label}-{stamp}-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn measure(dim: usize, n: usize, algorithm: Algorithm, queries: usize) -> Measurement {
    let dir_path = temp_dir("measure");
    let dir = dir_path.as_path();
    let mut cfg = IndexConfig::new(dim as u32, Metric::Cosine);
    cfg.algorithm = algorithm;
    cfg.hnsw.ef_construction = 200;
    cfg.hnsw.ef_search = 1024;
    let index = VectorIndex::create(dir, cfg).unwrap();
    let start = Instant::now();
    for i in 0..n {
        let v = vector(i, dim);
        index
            .insert(VectorId(i as u128 + 1), &v, Metadata::empty())
            .unwrap();
    }
    index.flush().unwrap();
    let build_ms = start.elapsed().as_secs_f64() * 1000.0;
    drop(index);

    let start = Instant::now();
    let reopened = VectorIndex::open(dir).unwrap();
    let open_ms = start.elapsed().as_secs_f64() * 1000.0;
    // Build the Exact ground-truth index once per (dim, N) and reuse it for
    // every query; rebuilding it per query would dominate the measurement.
    let exact_for_truth = if algorithm == Algorithm::Hnsw {
        let exact_dir_path = temp_dir("truth");
        let exact_dir = exact_dir_path.as_path();
        let mut cfg = IndexConfig::new(dim as u32, Metric::Cosine);
        cfg.algorithm = Algorithm::Exact;
        let index = VectorIndex::create(exact_dir, cfg).unwrap();
        for i in 0..n {
            let v = vector(i, dim);
            index
                .insert(VectorId(i as u128 + 1), &v, Metadata::empty())
                .unwrap();
        }
        Some(index)
    } else {
        None
    };
    let mut latencies = Vec::with_capacity(queries);
    let mut recall_hits = 0usize;
    let mut recall_total = 0usize;
    for q in 0..queries {
        let query = vector(n + q * 17 + 1, dim);
        let start = Instant::now();
        let hits = reopened.search(&query, 10).unwrap();
        latencies.push(start.elapsed());
        if let Some(truth_index) = exact_for_truth.as_ref() {
            let truth: std::collections::HashSet<_> = truth_index
                .search(&query, 10)
                .unwrap()
                .into_iter()
                .map(|h| h.id)
                .collect();
            recall_hits += hits.iter().filter(|h| truth.contains(&h.id)).count();
            recall_total += truth.len();
        }
    }
    let stats = reopened.stats().unwrap();
    drop(reopened);
    drop(exact_for_truth);
    let _ = std::fs::remove_dir_all(&dir_path);
    Measurement {
        build_ms,
        open_ms,
        p50_ms: percentile(latencies.clone(), 0.50).as_secs_f64() * 1000.0,
        p95_ms: percentile(latencies, 0.95).as_secs_f64() * 1000.0,
        recall: if algorithm == Algorithm::Exact {
            1.0
        } else {
            recall_hits as f64 / recall_total.max(1) as f64
        },
        disk: stats.disk_bytes,
        heap: stats.heap_bytes,
    }
}

fn arg_value(args: &[String], flag: &str) -> Option<usize> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
}

fn arg_list(args: &[String], flag: &str) -> Option<Vec<usize>> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .map(|v| v.split(',').filter_map(|x| x.parse().ok()).collect())
        .filter(|v: &Vec<usize>| !v.is_empty())
}

fn rustc_version() -> String {
    std::process::Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_else(|| "unknown".into())
        .trim()
        .into()
}

fn chrono_like_now() -> String {
    format!(
        "unix-seconds {}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    )
}
