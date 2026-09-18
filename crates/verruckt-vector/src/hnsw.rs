//! HNSW — Hierarchical Navigable Small World graphs.
//!
//! Self-contained implementation following Malkov & Yashunin, *Efficient and
//! robust approximate nearest neighbor search using Hierarchical Navigable
//! Small World graphs* (TPAMI 2018). See docs/HNSW.md for the algorithm
//! provenance, the implementation choices, and the deviations from the paper.
//!
//! Graph layout: every node owns a fixed-capacity slot block in one flat
//! `Vec<u32>` — layer 0 gets `2M` slots, each higher layer `M` slots, empty
//! slots hold `SENTINEL`. Fixed capacities make the structure trivially
//! serializable and cheaply traversable (no per-node allocations).
//!
//! Determinism: a node's layer count is derived from a ChaCha8 stream seeded
//! by `(config.seed, wal_seq)`, so replaying the WAL rebuilds the exact same
//! graph. Distance ties break by node id via a total order, so heap pops are
//! reproducible.

use crate::config::Metric;
use crate::distance::graph_distance;
use crate::error::{Result, VectorError};
use crate::metadata::{Metadata, MetadataFilter};
use crate::storage::VectorData;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::cell::RefCell;
use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;

/// Empty link slot.
pub(crate) const SENTINEL: u32 = u32::MAX;
/// Hard cap on layer counts (unreachable for realistic N and M).
pub(crate) const MAX_LEVEL: u32 = 63;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DistNode {
    pub dist: f32,
    pub node: u32,
}

// `Ord` is implemented manually (not derived) so the ordering is a *total*
// order over f32 via `total_cmp` — no NaN panic paths, deterministic pops.
impl Eq for DistNode {}

impl Ord for DistNode {
    fn cmp(&self, other: &Self) -> Ordering {
        // Closer first; lower node id breaks ties.
        self.dist
            .total_cmp(&other.dist)
            .then_with(|| self.node.cmp(&other.node))
    }
}

impl PartialOrd for DistNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// ---------------------------------------------------------------------------
// Visited scratch — epoch-stamped, thread-local
// ---------------------------------------------------------------------------

/// Epoch-stamped visited set: no per-search clearing, no unsafe interior
/// mutability. `stamps[node] == epoch` means "visited this round".
pub(crate) struct Scratch {
    stamps: Vec<u32>,
    epoch: u32,
}

impl Scratch {
    fn new() -> Scratch {
        Scratch {
            stamps: Vec::new(),
            epoch: 1,
        }
    }

    fn ensure(&mut self, nodes: usize) {
        if self.stamps.len() < nodes {
            // Growth invalidates nothing, but resetting all stamps keeps the
            // epoch bookkeeping trivially correct.
            self.stamps.clear();
            self.stamps.resize(nodes, 0);
            self.epoch = 1;
        }
    }

    fn fresh_epoch(&mut self) {
        if self.epoch >= u32::MAX - 1 {
            self.stamps.iter_mut().for_each(|stamp| *stamp = 0);
            self.epoch = 1;
        } else {
            self.epoch += 1;
        }
    }

    fn visited(&self, node: u32) -> bool {
        self.stamps[node as usize] == self.epoch
    }

    fn mark(&mut self, node: u32) {
        self.stamps[node as usize] = self.epoch;
    }
}

thread_local! {
    static SCRATCH: RefCell<Scratch> = RefCell::new(Scratch::new());
}

pub(crate) fn with_scratch<R>(nodes: usize, f: impl FnOnce(&mut Scratch) -> R) -> R {
    SCRATCH.with(|cell| {
        let mut scratch = cell.borrow_mut();
        scratch.ensure(nodes);
        f(&mut scratch)
    })
}

// ---------------------------------------------------------------------------
// Graph storage
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub(crate) struct HnswGraph {
    m: usize,
    m0: usize,
    node_count: u32,
    entry: Option<u32>,
    max_level: u32,
    levels: Vec<u32>,
    offsets: Vec<u32>,
    links: Vec<u32>,
}

impl HnswGraph {
    pub fn new(m: u32) -> HnswGraph {
        HnswGraph {
            m: m as usize,
            m0: m as usize * 2,
            node_count: 0,
            entry: None,
            max_level: 0,
            levels: Vec::new(),
            offsets: Vec::new(),
            links: Vec::new(),
        }
    }

    /// Rebuild from persisted parts, validating every invariant a corrupted
    /// file could violate. Offsets are recomputed, never trusted from disk.
    pub fn from_parts(
        m: u32,
        levels: Vec<u32>,
        links: Vec<u32>,
        entry: Option<u32>,
        max_level: u32,
    ) -> Result<HnswGraph> {
        let m = m as usize;
        let m0 = m * 2;
        let node_count = levels.len() as u32;
        if node_count == 0 {
            if !links.is_empty() {
                return Err(VectorError::corrupt(
                    "graph",
                    format!("{} links but no nodes", links.len()),
                ));
            }
            return Ok(HnswGraph::new(m as u32));
        }
        if m == 0 {
            return Err(VectorError::corrupt("graph", "m must be >= 1"));
        }
        let mut offsets = Vec::with_capacity(levels.len());
        let mut pos = 0usize;
        for &level in &levels {
            if level > MAX_LEVEL {
                return Err(VectorError::corrupt(
                    "graph",
                    format!("node level {level} exceeds {MAX_LEVEL}"),
                ));
            }
            offsets.push(pos as u32);
            pos += m0 + level as usize * m;
        }
        if pos != links.len() {
            return Err(VectorError::corrupt(
                "graph",
                format!("links length {} does not match node levels ({})", links.len(), pos),
            ));
        }
        for &link in &links {
            if link != SENTINEL && link >= node_count {
                return Err(VectorError::corrupt(
                    "graph",
                    format!("link target {link} out of range (node count {node_count})"),
                ));
            }
        }
        if let Some(entry) = entry {
            if entry >= node_count {
                return Err(VectorError::corrupt(
                    "graph",
                    format!("entry node {entry} out of range (node count {node_count})"),
                ));
            }
            if max_level != levels[entry as usize] {
                return Err(VectorError::corrupt(
                    "graph",
                    format!("max level {max_level} does not match entry level {}", levels[entry as usize]),
                ));
            }
        } else if max_level != 0 {
            return Err(VectorError::corrupt(
                "graph",
                format!("max level {max_level} set without entry node"),
            ));
        }
        Ok(HnswGraph {
            m,
            m0,
            node_count,
            entry,
            max_level,
            levels,
            offsets,
            links,
        })
    }

    pub fn node_count(&self) -> u32 {
        self.node_count
    }

    pub fn entry(&self) -> Option<u32> {
        self.entry
    }

    pub fn max_level(&self) -> u32 {
        self.max_level
    }

    pub fn m(&self) -> u32 {
        self.m as u32
    }

    #[allow(dead_code)]
    pub fn level_of(&self, node: u32) -> u32 {
        self.levels[node as usize]
    }

    pub fn levels(&self) -> &[u32] {
        &self.levels
    }

    pub fn links(&self) -> &[u32] {
        &self.links
    }

    pub fn links_bytes(&self) -> u64 {
        (self.links.len() * 4 + self.levels.len() * 4 + self.offsets.len() * 4) as u64
    }

    /// Allocate a node with `level` upper layers (node exists on layers
    /// `0..=level`). Nodes are added densely in row order.
    pub fn add_node(&mut self, node: u32, level: u32) {
        debug_assert_eq!(
            node, self.node_count,
            "graph nodes must be added densely in row order"
        );
        let level = level.min(MAX_LEVEL);
        let slots = self.m0 + level as usize * self.m;
        self.offsets.push(self.links.len() as u32);
        self.links.resize(self.links.len() + slots, SENTINEL);
        self.levels.push(level);
        self.node_count += 1;
        match self.entry {
            None => {
                self.entry = Some(node);
                self.max_level = level;
            }
            Some(_) if level > self.max_level => {
                self.entry = Some(node);
                self.max_level = level;
            }
            _ => {}
        }
    }

    pub fn layer_links(&self, node: u32, layer: u32) -> &[u32] {
        debug_assert!(
            layer <= self.levels[node as usize],
            "layer {layer} above node {node} level {}",
            self.levels[node as usize]
        );
        let start = self.offsets[node as usize] as usize;
        if layer == 0 {
            &self.links[start..start + self.m0]
        } else {
            let slice_start = start + self.m0 + (layer as usize - 1) * self.m;
            &self.links[slice_start..slice_start + self.m]
        }
    }

    fn layer_links_mut(&mut self, node: u32, layer: u32) -> &mut [u32] {
        debug_assert!(layer <= self.levels[node as usize]);
        let start = self.offsets[node as usize] as usize;
        let (lo, hi) = if layer == 0 {
            (start, start + self.m0)
        } else {
            let slice_start = start + self.m0 + (layer as usize - 1) * self.m;
            (slice_start, slice_start + self.m)
        };
        &mut self.links[lo..hi]
    }

    /// Overwrite a node's links on a layer (remaining slots become SENTINEL).
    pub fn set_links(&mut self, node: u32, layer: u32, neighbors: &[u32]) {
        let links = self.layer_links_mut(node, layer);
        debug_assert!(neighbors.len() <= links.len(), "too many links for slot block");
        links.fill(SENTINEL);
        links[..neighbors.len()].copy_from_slice(neighbors);
    }

    /// Add `target` to `node`'s links on `layer` if a free slot exists.
    fn try_push_link(&mut self, node: u32, layer: u32, target: u32) -> bool {
        let links = self.layer_links_mut(node, layer);
        match links.iter().position(|&link| link == SENTINEL) {
            Some(slot) => {
                links[slot] = target;
                true
            }
            None => false,
        }
    }
}

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

/// Everything a traversal needs to score nodes and apply result filters.
pub(crate) struct SearchCtx<'a> {
    pub vectors: &'a VectorData,
    pub meta: &'a [Metadata],
    pub alive: &'a [bool],
    pub dim: usize,
    pub metric: Metric,
    pub query: &'a [f32],
    pub filter: Option<&'a MetadataFilter>,
}

impl SearchCtx<'_> {
    #[inline]
    fn dist(&self, node: u32) -> f32 {
        graph_distance(self.metric, self.query, self.vectors.row(node as usize, self.dim))
    }

    /// May this node enter a result set? (live + metadata filter)
    #[inline]
    fn passes(&self, node: u32) -> bool {
        if !self.alive[node as usize] {
            return false;
        }
        match self.filter {
            None => true,
            Some(filter) => filter.matches(&self.meta[node as usize]),
        }
    }
}

/// SEARCH-LAYER (Algorithm 2). Returns up to `ef` result nodes passing the
/// ctx filter, sorted by distance asc. Tombstoned and filtered-out nodes are
/// still traversed — they keep the graph navigable — but never returned.
///
/// `budget` caps distance evaluations (used by filtered search so a highly
/// selective filter cannot degenerate into a full graph walk per attempt).
pub(crate) fn search_layer(
    ctx: &SearchCtx,
    graph: &HnswGraph,
    scratch: &mut Scratch,
    entries: &[DistNode],
    ef: usize,
    layer: u32,
    budget: Option<usize>,
) -> Vec<DistNode> {
    scratch.fresh_epoch();
    let mut candidates: BinaryHeap<Reverse<DistNode>> = BinaryHeap::new();
    let mut results: BinaryHeap<DistNode> = BinaryHeap::new();
    let mut evals = 0usize;

    for &entry in entries {
        if scratch.visited(entry.node) {
            continue;
        }
        scratch.mark(entry.node);
        candidates.push(Reverse(entry));
        if ctx.passes(entry.node) {
            results.push(entry);
        }
    }

    while let Some(Reverse(current)) = candidates.pop() {
        if results.len() >= ef {
            if let Some(worst) = results.peek() {
                if current.dist > worst.dist {
                    break;
                }
            }
        }
        if let Some(budget) = budget {
            if evals >= budget {
                break;
            }
        }
        for &neighbor in graph.layer_links(current.node, layer) {
            if neighbor == SENTINEL || scratch.visited(neighbor) {
                continue;
            }
            scratch.mark(neighbor);
            let dist = ctx.dist(neighbor);
            evals += 1;
            let better = match results.peek() {
                None => true,
                Some(worst) => dist < worst.dist,
            };
            if results.len() < ef || better {
                candidates.push(Reverse(DistNode { dist, node: neighbor }));
                if ctx.passes(neighbor) {
                    results.push(DistNode { dist, node: neighbor });
                    if results.len() > ef {
                        results.pop();
                    }
                }
            }
        }
    }

    let mut out: Vec<DistNode> = results.into_vec();
    out.sort_unstable();
    out
}

/// Full graph search: greedy descent from the entry point through upper
/// layers, then a full `ef`-wide search on layer 0. Returns ≤ k nodes.
pub(crate) fn graph_search(
    ctx: &SearchCtx,
    graph: &HnswGraph,
    k: usize,
    ef: usize,
    budget: Option<usize>,
) -> Vec<DistNode> {
    let Some(entry) = graph.entry() else {
        return Vec::new();
    };
    with_scratch(graph.node_count() as usize, |scratch| {
        let mut entry_point = DistNode {
            dist: ctx.dist(entry),
            node: entry,
        };
        for layer in (1..=graph.max_level()).rev() {
            let best = search_layer(ctx, graph, scratch, &[entry_point], 1, layer, None);
            if let Some(&first) = best.first() {
                entry_point = first;
            }
        }
        let mut hits = search_layer(ctx, graph, scratch, &[entry_point], ef.max(k), 0, budget);
        hits.truncate(k);
        hits
    })
}

// ---------------------------------------------------------------------------
// Construction
// ---------------------------------------------------------------------------

/// INSERT (Algorithm 1). `ctx.query` must be the *new node's* vector; the
/// ctx filter is ignored during construction.
pub(crate) fn graph_insert(
    graph: &mut HnswGraph,
    ctx: &SearchCtx,
    node: u32,
    level: u32,
    ef_construction: usize,
    heuristic: bool,
) {
    let old_entry = graph.entry();
    let old_max_level = graph.max_level();
    graph.add_node(node, level);
    let Some(entry) = old_entry else {
        return; // first node in the graph: nothing to connect to
    };

    let m = graph.m() as usize;
    let m0 = graph.m() as usize * 2;
    let mut entry_point = DistNode {
        dist: ctx.dist(entry),
        node: entry,
    };

    with_scratch(graph.node_count() as usize, |scratch| {
        // Greedy descent through layers above the new node's level.
        if level < old_max_level {
            for layer in ((level + 1)..=old_max_level).rev() {
                let best = search_layer(ctx, graph, scratch, &[entry_point], 1, layer, None);
                if let Some(&first) = best.first() {
                    entry_point = first;
                }
            }
        }

        let top = level.min(old_max_level);
        for layer in (0..=top).rev() {
            let candidates = search_layer(
                ctx,
                graph,
                scratch,
                &[entry_point],
                ef_construction,
                layer,
                None,
            );
            let candidates: Vec<DistNode> =
                candidates.into_iter().filter(|c| c.node != node).collect();
            let selected = select_neighbors(ctx, &candidates, m, heuristic);

            // Connect the new node (fresh slots, always room for ≤ M links).
            graph.set_links(node, layer, &selected);

            // Reverse links; shrink over-full neighbors by re-selection
            // relative to *their* vector.
            for &neighbor in &selected {
                if graph.try_push_link(neighbor, layer, node) {
                    continue;
                }
                let cap = if layer == 0 { m0 } else { m };
                let neighbor_ctx = SearchCtx {
                    vectors: ctx.vectors,
                    meta: ctx.meta,
                    alive: ctx.alive,
                    dim: ctx.dim,
                    metric: ctx.metric,
                    query: ctx.vectors.row(neighbor as usize, ctx.dim),
                    filter: None,
                };
                let mut current: Vec<DistNode> = graph
                    .layer_links(neighbor, layer)
                    .iter()
                    .filter(|&&link| link != SENTINEL)
                    .map(|&link| DistNode {
                        dist: graph_distance(
                            neighbor_ctx.metric,
                            neighbor_ctx.query,
                            neighbor_ctx.vectors.row(link as usize, neighbor_ctx.dim),
                        ),
                        node: link,
                    })
                    .collect();
                current.sort_unstable();
                let kept = select_neighbors(&neighbor_ctx, &current, cap, heuristic);
                graph.set_links(neighbor, layer, &kept);
            }

            if let Some(&first) = candidates.first() {
                entry_point = first;
            }
        }
    });
}

/// SELECT-NEIGHBORS-HEURISTIC (Algorithm 4) with keepPrunedConnections, or
/// plain top-M when `heuristic` is false. `candidates` must be sorted by
/// distance to the reference vector asc.
pub(crate) fn select_neighbors(
    ctx: &SearchCtx,
    candidates: &[DistNode],
    m: usize,
    heuristic: bool,
) -> Vec<u32> {
    if m == 0 {
        return Vec::new();
    }
    if !heuristic {
        return candidates.iter().take(m).map(|c| c.node).collect();
    }
    let mut selected: Vec<u32> = Vec::with_capacity(m);
    let mut discarded: Vec<u32> = Vec::new();
    for candidate in candidates {
        if selected.len() >= m {
            break;
        }
        let candidate_vector = ctx.vectors.row(candidate.node as usize, ctx.dim);
        let closer_than_all = selected.iter().all(|&chosen| {
            let chosen_vector = ctx.vectors.row(chosen as usize, ctx.dim);
            candidate.dist < graph_distance(ctx.metric, candidate_vector, chosen_vector)
        });
        if closer_than_all {
            selected.push(candidate.node);
        } else {
            discarded.push(candidate.node);
        }
    }
    // keepPrunedConnections: fill remaining capacity with the nearest pruned
    // candidates so sparse regions still get connected.
    let remaining = m.saturating_sub(selected.len());
    selected.extend(discarded.into_iter().take(remaining));
    selected
}

/// Deterministic layer count for an insert: derived from the configured seed
/// and the WAL sequence number, so crash replay rebuilds the same levels.
pub(crate) fn assign_level(seed: u64, seq: u64, m: u32) -> u32 {
    let mixed = mix_seed_seq(seed, seq);
    let mut rng = ChaCha8Rng::seed_from_u64(mixed);
    let uniform = rng.gen::<f64>().max(1e-300);
    let level_multiplier = 1.0 / (m.max(2) as f64).ln();
    let level = (-uniform.ln() * level_multiplier).floor();
    level.clamp(0.0, MAX_LEVEL as f64) as u32
}

fn mix_seed_seq(seed: u64, seq: u64) -> u64 {
    // splitmix64 finalizer over the mixed pair
    let mut z = seed ^ seq.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::VectorId;
    use crate::storage::{Rows, VectorData};

    fn build_test_graph(n: u32, dim: usize, seed: u64) -> (Rows, HnswGraph) {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let mut rows = Rows::empty(VectorData::empty_heap());
        let mut graph = HnswGraph::new(8);
        for i in 0..n {
            let vector: Vec<f32> = (0..dim).map(|_| rng.gen::<f32>() * 2.0 - 1.0).collect();
            let row = rows.insert(VectorId(i as u128), &vector, Metadata::empty());
            let level = assign_level(seed, i as u64, 8);
            let ctx = SearchCtx {
                vectors: &rows.vectors,
                meta: &rows.meta,
                alive: &rows.alive,
                dim,
                metric: Metric::L2,
                query: rows.vectors.row(row as usize, dim),
                filter: None,
            };
            graph_insert(&mut graph, &ctx, row, level, 64, true);
        }
        (rows, graph)
    }

    fn search_top1(rows: &Rows, graph: &HnswGraph, query: &[f32], dim: usize) -> Option<u32> {
        let ctx = SearchCtx {
            vectors: &rows.vectors,
            meta: &rows.meta,
            alive: &rows.alive,
            dim,
            metric: Metric::L2,
            query,
            filter: None,
        };
        graph_search(&ctx, graph, 1, 32, None).first().map(|h| h.node)
    }

    #[test]
    fn top1_matches_query_point() {
        let (rows, graph) = build_test_graph(200, 8, 42);
        for probe in [0u32, 7, 33, 100, 199] {
            let query = rows.copy_vector(probe, 8);
            assert_eq!(search_top1(&rows, &graph, &query, 8), Some(probe));
        }
    }

    #[test]
    fn construction_is_deterministic() {
        let (rows_a, graph_a) = build_test_graph(100, 6, 7);
        let (rows_b, graph_b) = build_test_graph(100, 6, 7);
        assert_eq!(graph_a.links(), graph_b.links());
        assert_eq!(graph_a.levels(), graph_b.levels());
        assert_eq!(graph_a.entry(), graph_b.entry());
        assert_eq!(rows_a.ids, rows_b.ids);
    }

    #[test]
    fn deleted_nodes_are_not_returned() {
        let (mut rows, graph) = build_test_graph(50, 4, 9);
        let victim = 10;
        let query = rows.copy_vector(victim, 4);
        rows.tombstone(victim);
        let ctx = SearchCtx {
            vectors: &rows.vectors,
            meta: &rows.meta,
            alive: &rows.alive,
            dim: 4,
            metric: Metric::L2,
            query: &query,
            filter: None,
        };
        let hits = graph_search(&ctx, &graph, 3, 32, None);
        assert!(hits.iter().all(|h| h.node != victim));
        assert!(!hits.is_empty());
    }

    #[test]
    fn from_parts_validates_links() {
        let (_, graph) = build_test_graph(10, 4, 3);
        let mut links = graph.links().to_vec();
        links[0] = 999; // out of range
        let err = HnswGraph::from_parts(graph.m(), graph.levels().to_vec(), links, graph.entry(), graph.max_level())
            .unwrap_err();
        assert!(matches!(err, VectorError::CorruptIndex { .. }));
    }
}
