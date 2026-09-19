//! Distance / similarity kernels.
//!
//! All kernels operate on contiguous `f32` slices and are written so the
//! optimizer can auto-vectorize them (simple accumulator loop, no pointer
//! chasing, fixed stride). A portable fallback *is* the implementation —
//! there is no hand-written SIMD and no `unsafe`.

use crate::config::Metric;

/// Euclidean norm.
#[inline]
pub fn norm(a: &[f32]) -> f32 {
    dot(a, a).sqrt()
}

/// Inner product of two equal-length slices.
#[inline]
pub fn dot(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());
    let mut acc = 0.0f32;
    let n = a.len().min(b.len());
    let mut i = 0;
    // Two independent accumulators help ILP / vectorization on wide dims.
    let mut acc0 = 0.0f32;
    let mut acc1 = 0.0f32;
    while i + 8 <= n {
        acc0 += a[i] * b[i] + a[i + 1] * b[i + 1] + a[i + 2] * b[i + 2] + a[i + 3] * b[i + 3];
        acc1 +=
            a[i + 4] * b[i + 4] + a[i + 5] * b[i + 5] + a[i + 6] * b[i + 6] + a[i + 7] * b[i + 7];
        i += 8;
    }
    acc += acc0 + acc1;
    while i < n {
        acc += a[i] * b[i];
        i += 1;
    }
    acc
}

/// Squared Euclidean distance.
#[inline]
pub fn l2_sq(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());
    let mut acc = 0.0f32;
    let n = a.len().min(b.len());
    let mut i = 0;
    let mut acc0 = 0.0f32;
    let mut acc1 = 0.0f32;
    while i + 8 <= n {
        for j in 0..4 {
            let d = a[i + j] - b[i + j];
            acc0 += d * d;
        }
        for j in 4..8 {
            let d = a[i + j] - b[i + j];
            acc1 += d * d;
        }
        i += 8;
    }
    acc += acc0 + acc1;
    while i < n {
        let d = a[i] - b[i];
        acc += d * d;
        i += 1;
    }
    acc
}

/// Cosine similarity without normalizing inputs (`dot / (|a| |b|)`).
/// Zero vectors yield `0.0` by convention (documented in README).
#[inline]
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let denom = norm(a) * norm(b);
    if denom <= f32::EPSILON || !denom.is_finite() {
        return 0.0;
    }
    dot(a, b) / denom
}

/// Normalize in place; returns false when the vector is (near) zero.
/// Zero vectors are left untouched.
#[inline]
pub fn normalize_in_place(v: &mut [f32]) -> bool {
    let n = norm(v);
    if n <= f32::EPSILON || !n.is_finite() {
        return false;
    }
    let inv = 1.0 / n;
    for component in v.iter_mut() {
        *component *= inv;
    }
    true
}

/// Distance used for graph traversal: **lower = closer**.
///
/// * Cosine (vectors pre-normalized): `1 - dot`
/// * Dot: `-dot`
/// * L2: squared euclidean distance (monotonic in true distance, cheaper)
#[inline]
pub fn graph_distance(metric: Metric, a: &[f32], b: &[f32]) -> f32 {
    match metric {
        Metric::Cosine => 1.0 - dot(a, b),
        Metric::Dot => -dot(a, b),
        Metric::L2 => l2_sq(a, b),
    }
}

/// Final public score for a distance metric: **higher = more similar**.
///
/// * Cosine: dot of normalized vectors (in `[-1, 1]`)
/// * Dot: raw inner product
/// * L2: `-euclidean_distance` (i.e. `0.0` = identical)
#[inline]
pub fn score(metric: Metric, a: &[f32], b: &[f32]) -> f32 {
    match metric {
        Metric::Cosine => dot(a, b),
        Metric::Dot => dot(a, b),
        Metric::L2 => -l2_sq(a, b).sqrt(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dot_basics() {
        assert!((dot(&[1.0, 2.0, 3.0], &[4.0, -5.0, 6.0]) - 12.0).abs() < 1e-6);
    }

    #[test]
    fn dot_matches_naive_on_odd_lengths() {
        for len in [1usize, 3, 7, 9, 15, 17, 31, 63] {
            let a: Vec<f32> = (0..len).map(|i| (i as f32) * 0.5 - 3.0).collect();
            let b: Vec<f32> = (0..len).map(|i| ((i * 7) % 13) as f32 - 6.0).collect();
            let naive = a.iter().zip(&b).map(|(x, y)| x * y).sum::<f32>();
            assert!(
                (dot(&a, &b) - naive).abs() < 1e-3,
                "len {len}: {} vs {naive}",
                dot(&a, &b)
            );
        }
    }

    #[test]
    fn l2_matches_naive() {
        let a = [1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0];
        let b = [9.0f32, 8.0, 7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0];
        let naive = a
            .iter()
            .zip(&b)
            .map(|(x, y)| (x - y) * (x - y))
            .sum::<f32>();
        assert!((l2_sq(&a, &b) - naive).abs() < 1e-4);
    }

    #[test]
    fn cosine_known_values() {
        assert!((cosine(&[1.0, 0.0], &[0.0, 1.0])).abs() < 1e-6);
        assert!((cosine(&[1.0, 1.0], &[1.0, 1.0]) - 1.0).abs() < 1e-6);
        assert!((cosine(&[1.0, 0.0], &[-1.0, 0.0]) + 1.0).abs() < 1e-6);
        assert_eq!(cosine(&[0.0, 0.0], &[1.0, 0.0]), 0.0);
    }

    #[test]
    fn normalize_unit() {
        let mut v = vec![3.0f32, 4.0];
        assert!(normalize_in_place(&mut v));
        assert!((norm(&v) - 1.0).abs() < 1e-6);
        let mut zero = vec![0.0f32; 4];
        assert!(!normalize_in_place(&mut zero));
        assert!(zero.iter().all(|component| *component == 0.0));
    }

    #[test]
    fn graph_distance_ordering() {
        let a = [1.0f32, 0.0];
        let near = [0.9f32, 0.1];
        let far = [0.0f32, 1.0];
        assert!(
            graph_distance(Metric::Cosine, &a, &near) < graph_distance(Metric::Cosine, &a, &far)
        );
        assert!(graph_distance(Metric::Dot, &a, &near) < graph_distance(Metric::Dot, &a, &far));
        assert!(graph_distance(Metric::L2, &a, &near) < graph_distance(Metric::L2, &a, &far));
        // Score is the reverse ordering.
        assert!(score(Metric::Cosine, &a, &near) > score(Metric::Cosine, &a, &far));
        assert!(score(Metric::L2, &a, &near) > score(Metric::L2, &a, &far));
    }
}
