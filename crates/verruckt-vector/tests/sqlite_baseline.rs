use rusqlite::{params, Connection};
use std::time::{Duration, Instant};

fn vector(seed: usize, dim: usize) -> Vec<f32> {
    let mut state = (seed as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    (0..dim)
        .map(|_| {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            ((state >> 33) as f32 / u32::MAX as f32) * 2.0 - 1.0
        })
        .collect()
}

fn blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

fn from_blob(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na <= f32::EPSILON || nb <= f32::EPSILON {
        0.0
    } else {
        dot / (na * nb)
    }
}

#[test]
fn sqlite_blob_baseline_real_measurements() {
    for &n in &[1_000usize, 10_000] {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("baseline.sqlite");
        let start = Instant::now();
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch("CREATE TABLE embeddings (id INTEGER PRIMARY KEY, dimensions INTEGER NOT NULL, vector BLOB NOT NULL);")
            .unwrap();
        let tx = conn.unchecked_transaction().unwrap();
        for i in 0..n {
            let v = vector(i, 384);
            tx.execute(
                "INSERT INTO embeddings(id, dimensions, vector) VALUES (?1, ?2, ?3)",
                params![i as i64, 384i64, blob(&v)],
            )
            .unwrap();
        }
        tx.commit().unwrap();
        let build_ms = start.elapsed().as_secs_f64() * 1000.0;
        drop(conn);

        let start = Instant::now();
        let conn = Connection::open(&db_path).unwrap();
        let mut stmt = conn
            .prepare("SELECT vector FROM embeddings ORDER BY id")
            .unwrap();
        let rows: Vec<Vec<f32>> = stmt
            .query_map([], |row| {
                let bytes: Vec<u8> = row.get(0)?;
                Ok(from_blob(&bytes))
            })
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        let open_ms = start.elapsed().as_secs_f64() * 1000.0;

        let mut latencies = Vec::new();
        for q in 0..10 {
            let query = vector(n + q * 17 + 1, 384);
            let start = Instant::now();
            let mut scores: Vec<f32> = rows.iter().map(|v| cosine(&query, v)).collect();
            scores.sort_by(|a, b| b.total_cmp(a));
            let _ = &scores[..10.min(scores.len())];
            latencies.push(start.elapsed());
        }
        latencies.sort_unstable();
        let p50 = latencies[latencies.len() / 2].as_secs_f64() * 1000.0;
        let p95 = latencies[((latencies.len() - 1) as f64 * 0.95).round() as usize].as_secs_f64()
            * 1000.0;
        let disk = std::fs::metadata(&db_path).unwrap().len();
        println!("sqlite baseline N={n} dim=384 build_ms={build_ms:.2} open_ms={open_ms:.2} p50_ms={p50:.4} p95_ms={p95:.4} disk={disk}");
    }
}

#[allow(dead_code)]
fn _duration_ms(value: Duration) -> f64 {
    value.as_secs_f64() * 1000.0
}
