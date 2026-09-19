//! Crash-recovery test child. Run with:
//!
//!   crash-child <mode> <index-dir> [seed]
//!
//! Modes simulate a process dying at specific points:
//!
//! * `wal-only`      — insert N vectors, flush WAL, hard-exit (no checkpoint).
//! * `wal-torn`      — insert N vectors, append a truncated WAL record by
//!   hand, hard-exit mid-frame.
//! * `mid-checkpoint`— insert N, then simulate dying during checkpoint by
//!   writing a partial new generation and exiting.
//!
//! The parent reopens the directory afterwards and asserts consistency.

use std::path::PathBuf;
use std::process::exit;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: crash-child <mode> <index-dir> [seed]");
        exit(2);
    }
    let mode = args[1].as_str();
    let dir = PathBuf::from(&args[2]);
    let seed: u64 = args.get(3).map(|s| s.parse().unwrap()).unwrap_or(7);

    match mode {
        "wal-only" => wal_only(&dir, seed),
        "wal-torn" => wal_torn(&dir, seed),
        "mid-checkpoint" => mid_checkpoint(&dir, seed),
        other => {
            eprintln!("unknown mode {other}");
            exit(2);
        }
    }
}

/// Deterministic pseudo-random vectors (no rand dependency in the bin).
fn vector(i: u64, dim: usize) -> Vec<f32> {
    let mut state = i.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(1);
    (0..dim)
        .map(|_| {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((state >> 33) as f32 / u32::MAX as f32) * 2.0 - 1.0
        })
        .collect()
}

fn build_index(dir: &std::path::Path, dim: u32) -> verruckt_vector::VectorIndex {
    use verruckt_vector::*;
    if manifest_exists(dir) {
        VectorIndex::open(dir).expect("open existing index")
    } else {
        VectorIndex::create(dir, IndexConfig::new(dim, Metric::Cosine)).expect("create index")
    }
}

fn manifest_exists(dir: &std::path::Path) -> bool {
    dir.join("manifest.bin").exists()
}

fn wal_only(dir: &std::path::Path, seed: u64) {
    let index = build_index(dir, 16);
    for i in 0..50u64 {
        index
            .insert(
                verruckt_vector::VectorId(seed as u128 * 1000 + i as u128),
                &vector(i, 16),
                verruckt_vector::Metadata::empty(),
            )
            .expect("insert");
    }
    index.flush().expect("flush wal");
    // Hard exit: no checkpoint, no close, nothing graceful.
    exit(0);
}

fn wal_torn(dir: &std::path::Path, seed: u64) {
    let index = build_index(dir, 16);
    for i in 0..30u64 {
        index
            .insert(
                verruckt_vector::VectorId(seed as u128 * 1000 + i as u128),
                &vector(i, 16),
                verruckt_vector::Metadata::empty(),
            )
            .expect("insert");
    }
    index.flush().expect("flush wal");
    drop(index);
    // Simulate dying mid-WAL-append: splice a half-written frame (valid magic,
    // truncated body) onto the log tail.
    let wal = dir.join("wal.log");
    let mut bytes = std::fs::read(&wal).expect("read wal");
    // A valid frame header claiming a payload that never arrives.
    bytes.extend_from_slice(b"VWL1");
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&1000u64.to_le_bytes());
    bytes.extend_from_slice(&[0xAB; 40]); // partial payload, no checksum
    std::fs::write(&wal, &bytes).expect("write torn wal");
    exit(0);
}

fn mid_checkpoint(dir: &std::path::Path, seed: u64) {
    let index = build_index(dir, 16);
    for i in 0..40u64 {
        index
            .insert(
                verruckt_vector::VectorId(seed as u128 * 1000 + i as u128),
                &vector(i, 16),
                verruckt_vector::Metadata::empty(),
            )
            .expect("insert");
    }
    index.flush().expect("flush wal");
    drop(index);
    // Simulate dying partway through a checkpoint: a fresh generation's
    // vectors file exists but is truncated, and no manifest was committed.
    // (The engine would have written vectors.N.dat fully; we fake the torn
    // state directly on disk.)
    let entries = std::fs::read_dir(dir).expect("read dir");
    let mut max_gen = 0u64;
    for entry in entries.flatten() {
        if let Some(name) = entry.file_name().to_str() {
            if let Some(rest) = name.strip_prefix("vectors.") {
                if let Some(gen_str) = rest.strip_suffix(".dat") {
                    if let Ok(gen) = gen_str.parse::<u64>() {
                        max_gen = max_gen.max(gen);
                    }
                }
            }
        }
    }
    let partial = dir.join(format!("vectors.{}.dat", max_gen + 1));
    std::fs::write(&partial, b"VVD1\x01\x00\x00\x00").expect("write partial segment");
    exit(0);
}
