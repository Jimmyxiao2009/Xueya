//! Concurrency tests: many searches concurrent with inserts/deletes must
//! never panic, never observe a half-applied mutation, and always return
//! internally consistent results (see README §Concurrency).

use std::sync::Arc;
use std::thread;

use verruckt_vector::{IndexConfig, Metadata, Metric, VectorId, VectorIndex};

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
fn concurrent_searches_and_writers_never_panic() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = IndexConfig::new(32, Metric::Cosine);
    config.exact_threshold = 16; // small enough to cross the exact/hnsw boundary mid-test
    let index = Arc::new(VectorIndex::create(dir.path(), config).unwrap());

    // Seed rows 0..64.
    for i in 0..64u128 {
        index
            .insert(
                VectorId(i + 1),
                &lcg_vec(i as u64 + 1, 32),
                Metadata::empty(),
            )
            .unwrap();
    }

    let writer = {
        let index = Arc::clone(&index);
        thread::spawn(move || {
            for i in 0..200u128 {
                if i % 5 == 4 {
                    index.delete(VectorId((i % 64) + 1)).unwrap();
                } else {
                    index
                        .insert(
                            VectorId(i + 1000),
                            &lcg_vec(i as u64 + 5000, 32),
                            Metadata::empty(),
                        )
                        .unwrap();
                }
            }
        })
    };

    let readers: Vec<_> = (0..4)
        .map(|reader_id| {
            let index = Arc::clone(&index);
            thread::spawn(move || {
                for step in 0..200 {
                    let query = lcg_vec(reader_id * 1000 + step, 32);
                    // May error only with IndexClosed (never happens here);
                    // must never panic and never return >k results.
                    if let Ok(hits) = index.search(&query, 5) {
                        assert!(hits.len() <= 5);
                    }
                    if let Ok(hits) = index.search(&query, 10) {
                        // Scores must be non-increasing.
                        for pair in hits.windows(2) {
                            assert!(
                                pair[0].score >= pair[1].score
                                    || (pair[0].score - pair[1].score).abs() < 1e-6,
                                "scores must be sorted: {} then {}",
                                pair[0].score,
                                pair[1].score
                            );
                        }
                    }
                }
            })
        })
        .collect();

    writer.join().expect("writer thread");
    for reader in readers {
        reader.join().expect("reader thread");
    }
    index.flush().unwrap();
}

#[test]
fn concurrent_flush_and_compact_with_reads() {
    let dir = tempfile::tempdir().unwrap();
    let index =
        Arc::new(VectorIndex::create(dir.path(), IndexConfig::new(16, Metric::Cosine)).unwrap());
    for i in 0..32u128 {
        index
            .insert(
                VectorId(i + 1),
                &lcg_vec(i as u64 + 1, 16),
                Metadata::empty(),
            )
            .unwrap();
    }

    let churner = {
        let index = Arc::clone(&index);
        thread::spawn(move || {
            for round in 0..5 {
                for i in 0..16u128 {
                    index
                        .insert(
                            VectorId(10_000 + (round as u128) * 100 + i),
                            &lcg_vec(10_000 + round as u64 * 100 + i as u64, 16),
                            Metadata::empty(),
                        )
                        .unwrap();
                }
                index.flush().unwrap();
            }
            index.compact().unwrap();
        })
    };

    let readers: Vec<_> = (0..3)
        .map(|reader_id| {
            let index = Arc::clone(&index);
            thread::spawn(move || {
                for step in 0..300 {
                    let query = lcg_vec(reader_id * 7777 + step, 16);
                    let _ = index.search(&query, 3);
                }
            })
        })
        .collect();

    churner.join().expect("churner thread");
    for reader in readers {
        reader.join().expect("reader thread");
    }
}

#[test]
fn batch_insert_is_atomic_on_validation_failure() {
    let dir = tempfile::tempdir().unwrap();
    let index = VectorIndex::create(dir.path(), IndexConfig::new(16, Metric::Cosine)).unwrap();
    // Batch with a bad vector in the middle: nothing must be applied.
    let good = lcg_vec(1, 16);
    let bad = vec![0.1f32; 8]; // wrong dimension
    let result = index.insert_batch([
        (VectorId(1), good.as_slice(), Metadata::empty()),
        (VectorId(2), bad.as_slice(), Metadata::empty()),
        (VectorId(3), good.as_slice(), Metadata::empty()),
    ]);
    assert!(result.is_err());
    assert_eq!(index.stats().unwrap().live_count, 0);
    // The WAL must not contain a half-applied batch.
    index.flush().unwrap();
    drop(index);
    let reopened = VectorIndex::open(dir.path()).unwrap();
    assert_eq!(reopened.stats().unwrap().live_count, 0);
}
