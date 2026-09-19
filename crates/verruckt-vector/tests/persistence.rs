//! Persistence, crash-recovery, and corruption integration tests.

use std::path::Path;
use std::process::Command;

use verruckt_vector::{
    Algorithm, IndexConfig, Metadata, MetadataField, MetadataFilter, Metric, VectorError, VectorId,
    VectorIndex,
};

fn config(dim: u32) -> IndexConfig {
    IndexConfig::new(dim, Metric::Cosine)
}

/// Deterministic unit-ish vector with a dominant axis (see engine tests).
fn vec_of(seed: usize, dim: usize) -> Vec<f32> {
    let mut v = vec![0.01; dim];
    v[seed % dim] = 1.0;
    v
}

/// Deterministic dense vector (LCG): unique-ish per seed, well spread.
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

#[test]
fn insert_10k_close_reopen_search_matches() {
    let dir = tempfile::tempdir().unwrap();
    let index = VectorIndex::create(dir.path(), config(64)).unwrap();

    let batch: Vec<(VectorId, Vec<f32>, Metadata)> = (0..10_000u64)
        .map(|i| {
            (
                VectorId(i as u128 + 1),
                lcg_vec(i + 1, 64),
                Metadata::empty(),
            )
        })
        .collect();
    let refs: Vec<(VectorId, &[f32], Metadata)> = batch
        .iter()
        .map(|(id, v, m)| (*id, v.as_slice(), m.clone()))
        .collect();
    index.insert_batch(refs).unwrap();
    index.flush().unwrap();
    drop(index);

    let reopened = VectorIndex::open(dir.path()).unwrap();
    let stats = reopened.stats().unwrap();
    assert_eq!(stats.live_count, 10_000);
    assert_eq!(stats.deleted_count, 0);

    // Ground truth: query with id 5000's exact vector → top1 must be id 5000.
    let needle = lcg_vec(5000, 64);
    let hits = reopened.search(&needle, 5).unwrap();
    assert_eq!(hits[0].id, VectorId(5000));
    // And re-reopen gives identical results (checkpoint-on-open stability).
    drop(reopened);
    let reopened2 = VectorIndex::open(dir.path()).unwrap();
    let hits2 = reopened2.search(&needle, 5).unwrap();
    assert_eq!(hits, hits2);
}

#[test]
fn crash_wal_only_recovers_all_inserts() {
    let dir = tempfile::tempdir().unwrap();
    let status = run_crash_child("wal-only", dir.path());
    assert!(status.success(), "crash child failed: {status:?}");

    let index = VectorIndex::open(dir.path()).unwrap();
    let stats = index.stats().unwrap();
    assert_eq!(stats.live_count, 50, "all WAL inserts must be recovered");
    assert_eq!(stats.deleted_count, 0);
    // Search still works on the recovered state.
    let hits = index.search(&vec_of(3, 16), 5).unwrap();
    assert!(!hits.is_empty());
}

#[test]
fn crash_torn_wal_tail_recovers_prefix() {
    let dir = tempfile::tempdir().unwrap();
    let status = run_crash_child("wal-torn", dir.path());
    assert!(status.success(), "crash child failed: {status:?}");

    // The torn frame must be dropped; the 30 complete records must survive.
    let index = VectorIndex::open(dir.path()).unwrap();
    let stats = index.stats().unwrap();
    assert_eq!(stats.live_count, 30);
    // The index must remain usable after recovery.
    for i in 0..16u128 {
        index
            .insert(
                VectorId(50_000 + i),
                &vec_of(i as usize, 16),
                Metadata::empty(),
            )
            .unwrap();
    }
    index.flush().unwrap();
    drop(index);
    let reopened = VectorIndex::open(dir.path()).unwrap();
    assert_eq!(reopened.stats().unwrap().live_count, 46);
}

#[test]
fn crash_mid_checkpoint_keeps_previous_generation() {
    let dir = tempfile::tempdir().unwrap();
    let status = run_crash_child("mid-checkpoint", dir.path());
    assert!(status.success(), "crash child failed: {status:?}");

    // The manifest was never updated, so open must fall back to the last
    // committed generation and replay the WAL on top.
    let index = VectorIndex::open(dir.path()).unwrap();
    let stats = index.stats().unwrap();
    assert_eq!(stats.live_count, 40, "WAL replay after partial checkpoint");
    // Stale partial segment files must be cleaned up by the next checkpoint.
    index.compact().unwrap();
    let stats = index.stats().unwrap();
    assert_eq!(stats.live_count, 40);
    assert_eq!(stats.row_count, 40);
}

#[test]
fn corrupt_manifest_magic_is_rejected() {
    let dir = make_index(10);
    let path = dir.path().join("manifest.bin");
    let mut bytes = std::fs::read(&path).unwrap();
    bytes[0] ^= 0xFF;
    std::fs::write(&path, &bytes).unwrap();
    assert_corrupt(dir.path());
}

#[test]
fn corrupt_manifest_version_is_unsupported() {
    let dir = make_index(10);
    let path = dir.path().join("manifest.bin");
    let mut bytes = std::fs::read(&path).unwrap();
    // Flip payload format_version (first u32 after the 16-byte frame header).
    bytes[16] = 0x63;
    std::fs::write(&path, &bytes).unwrap();
    match VectorIndex::open(dir.path()) {
        Err(VectorError::CorruptIndex { .. }) | Err(VectorError::UnsupportedVersion { .. }) => {}
        other => panic!("expected corrupt/unsupported, got {other:?}"),
    }
}

#[test]
fn truncated_vectors_segment_is_rejected() {
    let dir = make_index_with_checkpoint(50);
    let vectors = newest_segment(dir.path(), "vectors");
    let mut bytes = std::fs::read(&vectors).unwrap();
    bytes.truncate(bytes.len() / 2);
    std::fs::write(&vectors, &bytes).unwrap();
    assert_corrupt(dir.path());
}

#[test]
fn truncated_graph_segment_is_rejected() {
    let dir = make_index_with_checkpoint(50);
    let graph = newest_segment(dir.path(), "graph");
    let mut bytes = std::fs::read(&graph).unwrap();
    bytes.truncate(bytes.len() - 7);
    std::fs::write(&graph, &bytes).unwrap();
    assert_corrupt(dir.path());
}

#[test]
fn bitflip_in_ids_segment_is_rejected() {
    let dir = make_index_with_checkpoint(50);
    let ids = newest_segment(dir.path(), "ids");
    let mut bytes = std::fs::read(&ids).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 0x55; // inside the checksum → mismatch, not truncation
    std::fs::write(&ids, &bytes).unwrap();
    assert_corrupt(dir.path());
}

#[test]
fn tampered_vectors_checksum_is_rejected() {
    let dir = make_index_with_checkpoint(50);
    let vectors = newest_segment(dir.path(), "vectors");
    let mut bytes = std::fs::read(&vectors).unwrap();
    // Flip a payload byte without touching the checksum trailer.
    bytes[30] ^= 0xFF;
    std::fs::write(&vectors, &bytes).unwrap();
    assert_corrupt(dir.path());
}

#[test]
fn wal_midlog_corruption_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let index = VectorIndex::create(dir.path(), config(16)).unwrap();
    for i in 0..20u128 {
        index
            .insert(VectorId(i + 1), &vec_of(i as usize, 16), Metadata::empty())
            .unwrap();
    }
    // Do not flush: the records must remain in wal.log for this corruption test.
    drop(index);
    // Corrupt the WAL payload after several complete records.
    let wal = dir.path().join("wal.log");
    let mut bytes = std::fs::read(&wal).unwrap();
    assert!(
        bytes.len() > 60,
        "WAL should contain uncheckpointed inserts"
    );
    bytes[60] ^= 0xFF;
    std::fs::write(&wal, &bytes).unwrap();
    match VectorIndex::open(dir.path()) {
        Err(VectorError::CorruptIndex { .. }) | Err(VectorError::UnsupportedVersion { .. }) => {}
        other => panic!("expected corrupt, got {other:?}"),
    }
}

#[test]
fn delete_survives_checkpoint_and_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let index = VectorIndex::create(dir.path(), config(16)).unwrap();
    for i in 0..20u128 {
        index
            .insert(VectorId(i + 1), &vec_of(i as usize, 16), Metadata::empty())
            .unwrap();
    }
    index.delete(VectorId(5)).unwrap();
    index.delete(VectorId(9)).unwrap();
    index.compact().unwrap(); // forces checkpoint + WAL truncate
    drop(index);

    let reopened = VectorIndex::open(dir.path()).unwrap();
    let hits = reopened.search(&vec_of(5, 16), 20).unwrap();
    assert!(hits
        .iter()
        .all(|hit| hit.id != VectorId(5) && hit.id != VectorId(9)));
    assert_eq!(reopened.stats().unwrap().live_count, 18);
}

#[test]
fn filtered_search_after_reopen_matches_exact() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = config(16);
    cfg.algorithm = Algorithm::Exact;
    let index = VectorIndex::create(dir.path(), cfg).unwrap();
    let route = VectorId(777);
    for i in 0..40u128 {
        let metadata = Metadata {
            route_id: Some(if i % 2 == 0 { route } else { VectorId(888) }),
            ..Metadata::empty()
        };
        index
            .insert(VectorId(i + 1), &vec_of(i as usize, 16), metadata)
            .unwrap();
    }
    index.flush().unwrap();
    drop(index);

    let reopened = VectorIndex::open(dir.path()).unwrap();
    let filter = MetadataFilter::new().route_id_eq(route);
    let hits = reopened
        .search_filtered(&vec_of(10, 16), 100, Some(&filter))
        .unwrap();
    assert!(!hits.is_empty());
    assert!(hits.iter().all(|hit| hit.metadata.route_id == Some(route)));

    // knowledge_point filter with in-set semantics.
    let filter = MetadataFilter::new().knowledge_point_id_in([VectorId(1), VectorId(2)]);
    let hits = reopened
        .search_filtered(&vec_of(1, 16), 10, Some(&filter))
        .unwrap();
    assert!(hits.is_empty(), "no kp ids were ever set");

    let filter = MetadataFilter::new()
        .eq(
            MetadataField::RouteId,
            verruckt_vector::FilterValue::Id(route),
        )
        .kind_eq("chunk");
    let hits = reopened
        .search_filtered(&vec_of(1, 16), 10, Some(&filter))
        .unwrap();
    assert!(hits.is_empty(), "no kind was ever set");
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn make_index(n: usize) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let index = VectorIndex::create(dir.path(), config(16)).unwrap();
    for i in 0..n as u128 {
        index
            .insert(VectorId(i + 1), &vec_of(i as usize, 16), Metadata::empty())
            .unwrap();
    }
    index.flush().unwrap();
    dir
}

/// Like make_index but forces a checkpoint so segments are non-trivial.
fn make_index_with_checkpoint(n: usize) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let index = VectorIndex::create(dir.path(), config(16)).unwrap();
    for i in 0..n as u128 {
        index
            .insert(VectorId(i + 1), &vec_of(i as usize, 16), Metadata::empty())
            .unwrap();
    }
    index.compact().unwrap();
    dir
}

fn newest_segment(dir: &Path, stem: &str) -> std::path::PathBuf {
    let mut best: Option<(u64, std::path::PathBuf)> = None;
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let prefix = format!("{stem}.");
        if let Some(rest) = name.strip_prefix(&prefix) {
            if let Some(gen_str) = rest.strip_suffix(".dat") {
                if let Ok(gen) = gen_str.parse::<u64>() {
                    if best.as_ref().map(|(g, _)| gen > *g).unwrap_or(true) {
                        best = Some((gen, entry.path()));
                    }
                }
            }
        }
    }
    best.expect("at least one segment").1
}

fn assert_corrupt(dir: &Path) {
    match VectorIndex::open(dir) {
        Err(VectorError::CorruptIndex { .. }) | Err(VectorError::UnsupportedVersion { .. }) => {}
        Err(other) => panic!("expected corrupt index, got {other:?}"),
        Ok(_) => panic!("corrupt index opened successfully"),
    }
}

fn run_crash_child(mode: &str, dir: &Path) -> std::process::ExitStatus {
    let exe = env!("CARGO_BIN_EXE_crash-child");
    Command::new(exe)
        .arg(mode)
        .arg(dir)
        .arg("7")
        .status()
        .expect("spawn crash child")
}
