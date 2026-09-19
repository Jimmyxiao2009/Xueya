//! Exact (brute-force) search — the ground truth and the small-index fast
//! path. A single linear scan over the flat vector arena with an optional
//! metadata filter, keeping the top-k in a bounded heap.

use crate::config::Metric;
use crate::distance::score;
use crate::metadata::{Metadata, MetadataFilter};
use crate::storage::Rows;
use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// A scored row. Ordering is a total order (score desc, row asc) so results
/// are deterministic even under ties.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Scored {
    pub score: f32,
    pub row: u32,
}

impl Eq for Scored {}

impl Ord for Scored {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher score first; lower row id breaks ties.
        other
            .score
            .total_cmp(&self.score)
            .then_with(|| self.row.cmp(&other.row))
    }
}

impl PartialOrd for Scored {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Scan every live row and return the top-`k` by score (desc, row asc).
pub(crate) fn exact_search(
    rows: &Rows,
    dim: usize,
    metric: Metric,
    query: &[f32],
    k: usize,
    filter: Option<&MetadataFilter>,
) -> Vec<Scored> {
    let meta: &[Metadata] = &rows.meta;
    let mut heap: BinaryHeap<Scored> = BinaryHeap::with_capacity(k.min(64));
    for (row, (alive, metadata)) in rows.alive.iter().zip(meta.iter()).enumerate() {
        if !alive {
            continue;
        }
        if let Some(filter) = filter {
            if !filter.matches(metadata) {
                continue;
            }
        }
        let vector = rows.vectors.row(row, dim);
        let hit = Scored {
            score: score(metric, query, vector),
            row: row as u32,
        };
        if heap.len() < k {
            heap.push(hit);
        } else if let Some(worst) = heap.peek() {
            // `Scored`'s max-heap order is "worst on top", so replace the
            // root only when the new hit is strictly better (smaller in the
            // reversed `Ord` ordering).
            if hit.cmp(worst) == Ordering::Less {
                heap.pop();
                heap.push(hit);
            }
        }
    }
    let mut hits: Vec<Scored> = heap.into_vec();
    hits.sort_unstable();
    hits
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::VectorId;
    use crate::storage::{Rows, VectorData};

    fn rows_with(vectors: &[&[f32]]) -> Rows {
        let mut rows = Rows::empty(VectorData::empty_heap());
        for (i, v) in vectors.iter().enumerate() {
            rows.insert(VectorId(i as u128), v, Metadata::empty());
        }
        rows
    }

    #[test]
    fn top_k_ordering_and_ties() {
        let rows = rows_with(&[
            &[1.0, 0.0],
            &[0.0, 1.0],
            &[1.0, 0.0], // ties with row 0 on cosine
            &[0.2, 0.1],
        ]);
        let hits = exact_search(&rows, 2, Metric::Cosine, &[1.0, 0.0], 4, None);
        let ids: Vec<u32> = hits.iter().map(|h| h.row).collect();
        // Row 3 scores 0.2, row 1 scores 0.0, so full ordering is 0, 2, 3, 1.
        assert_eq!(ids, vec![0, 2, 3, 1]);
    }

    #[test]
    fn k_larger_than_live_count_returns_all() {
        let rows = rows_with(&[&[1.0, 0.0], &[0.0, 1.0]]);
        let hits = exact_search(&rows, 2, Metric::Cosine, &[1.0, 0.0], 10, None);
        assert_eq!(hits.len(), 2);
        assert!(hits[0].score > hits[1].score);
    }
}
