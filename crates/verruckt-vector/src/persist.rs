//! On-disk layout (see docs/FORMAT.md):
//!
//! ```text
//! <dir>/manifest.bin            magic VVM1 — config + generation + checksums
//! <dir>/vectors.<gen>.dat       magic VVD1 — flat little-endian f32 rows
//! <dir>/graph.<gen>.dat         magic VVG1 — HNSW levels + links + entry
//! <dir>/ids.<gen>.dat           magic VII1 — external ids (16B each)
//! <dir>/meta.<gen>.dat          magic VVM2 — per-row metadata + alive bitmask
//! <dir>/wal.log                 magic VWL1 — frame-per-record WAL
//! <dir>/*.tmp                   staging files for atomic replacement
//! ```
//!
//! Every checkpoint/compaction writes a **new generation** of segment files
//! and commits it by atomically renaming the manifest; the previous
//! generation is deleted only afterwards. A crash at any point therefore
//! leaves the manifest pointing at a complete, checksummed generation.
//!
//! All data files use the codec frame `[magic][version=1][len][payload][fnv1a64]`,
//! little-endian throughout; nothing is ever a memory dump of a Rust struct.
//!
//! A row is **live** iff its bit is set in the meta.dat alive bitmask; its
//! external id is mapped in the in-memory idmap only while it is live.
//!
//! # Unsafe policy
//!
//! The crate is `#![forbid(unsafe_code)]` everywhere *except* this module,
//! which contains exactly one `unsafe` call: `Mmap::map` in
//! `mmap_segment` (any mmap is inherently unsafe). It carries a full inline
//! SAFETY argument and is the only audited unsafe call in the crate; see
//! docs/FORMAT.md §Safety.

// One audited exception to the crate-wide forbid(unsafe_code): the mmap call
// below. Everything else in this file is safe code.
#![allow(unsafe_code)]

use crate::codec::{frame, Reader, Writer};
use crate::config::{Algorithm, EmbeddingSpace, IndexConfig, Metric};
use crate::error::{Result, VectorError};
use crate::hnsw::{HnswGraph, MAX_LEVEL};
use crate::id::VectorId;
use crate::metadata::{Metadata, MAX_KIND_LEN};
use crate::storage::{Rows, VectorData};
use memmap2::Mmap;
use std::path::{Path, PathBuf};

pub(crate) const MANIFEST_MAGIC: [u8; 4] = *b"VVM1";
pub(crate) const VECTORS_MAGIC: [u8; 4] = *b"VVD1";
pub(crate) const GRAPH_MAGIC: [u8; 4] = *b"VVG1";
pub(crate) const IDS_MAGIC: [u8; 4] = *b"VII1";
pub(crate) const META_MAGIC: [u8; 4] = *b"VVM2";
pub(crate) const WAL_MAGIC: [u8; 4] = *b"VWL1";

/// Segment file stems, in manifest-checksum order.
const SEGMENT_STEMS: [&str; 4] = ["vectors", "graph", "ids", "meta"];

/// Segment file name for a generation, e.g. `vectors.7.dat`.
pub(crate) fn segment_file(stem: &str, generation: u64) -> String {
    format!("{stem}.{generation}.dat")
}

/// Remove segment files of every generation except `keep`, plus stale `.tmp`
/// staging files. Best effort — a failure to delete is not fatal (the file is
/// simply garbage until the next cleanup).
pub(crate) fn cleanup_stale_segments(dir: &Path, keep: u64) -> Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return Ok(()),
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if name.ends_with(".tmp") {
            let _ = std::fs::remove_file(entry.path());
            continue;
        }
        for stem in SEGMENT_STEMS {
            let prefix = format!("{stem}.");
            if let Some(rest) = name.strip_prefix(&prefix) {
                if let Some(gen_str) = rest.strip_suffix(".dat") {
                    if gen_str.parse::<u64>().is_ok() && gen_str.parse::<u64>() != Ok(keep) {
                        let _ = std::fs::remove_file(entry.path());
                    }
                }
            }
        }
    }
    Ok(())
}

/// Total size of every file under the index directory (stats only).
pub(crate) fn directory_size(dir: &Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() {
                    total += meta.len();
                }
            }
        }
    }
    total
}

// ---------------------------------------------------------------------------
// Manifest
// ---------------------------------------------------------------------------

/// The durable "commit record" of an index. Written last at checkpoint; if it
/// is absent or stale, the index opens at the previous checkpoint instead.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Manifest {
    pub format_version: u32,
    pub generation: u64,
    pub dimensions: u32,
    pub metric: Metric,
    pub algorithm: Algorithm,
    pub m: u32,
    pub ef_construction: u32,
    pub ef_search: u32,
    pub heuristic: bool,
    pub seed: u64,
    pub exact_threshold: u64,
    pub row_count: u32,
    pub live_count: u64,
    pub deleted_count: u64,
    pub max_seq: u64,
    pub graph_entry: Option<u32>,
    pub graph_max_level: u32,
    pub vectors_checksum: u64,
    pub graph_checksum: u64,
    pub ids_checksum: u64,
    pub meta_checksum: u64,
    pub created_at_unix: i64,
    pub embedding_space: EmbeddingSpace,
}

impl Manifest {
    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut w = Writer::with_capacity(160);
        w.u32(self.format_version);
        w.u64(self.generation);
        w.u32(self.dimensions);
        w.u8(self.metric.code());
        w.u8(self.algorithm.code());
        w.u32(self.m);
        w.u32(self.ef_construction);
        w.u32(self.ef_search);
        w.u8(u8::from(self.heuristic));
        w.u64(self.seed);
        w.u64(self.exact_threshold);
        w.u32(self.row_count);
        w.u64(self.live_count);
        w.u64(self.deleted_count);
        w.u64(self.max_seq);
        match self.graph_entry {
            Some(entry) => {
                w.u8(1);
                w.u32(entry);
            }
            None => {
                w.u8(0);
            }
        }
        w.u32(self.graph_max_level);
        w.u64(self.vectors_checksum);
        w.u64(self.graph_checksum);
        w.u64(self.ids_checksum);
        w.u64(self.meta_checksum);
        w.i64(self.created_at_unix);
        w.short_str(&self.embedding_space.provider);
        w.short_str(&self.embedding_space.model);
        w.short_str(&self.embedding_space.source_hash);
        frame::encode(&MANIFEST_MAGIC, frame::FORMAT_VERSION, &w.into_bytes())
    }

    pub(crate) fn decode(data: &[u8]) -> Result<Manifest> {
        let payload = frame::decode_v1(&MANIFEST_MAGIC, "manifest", data)?;
        let mut r = Reader::new(payload, "manifest");
        let format_version = r.u32()?;
        if format_version != frame::FORMAT_VERSION {
            return Err(VectorError::UnsupportedVersion {
                found: format_version,
                supported: frame::FORMAT_VERSION,
            });
        }
        let generation = r.u64()?;
        let dimensions = r.u32()?;
        let metric = Metric::from_code(r.u8()?)?;
        let algorithm = Algorithm::from_code(r.u8()?)?;
        let m = r.u32()?;
        let ef_construction = r.u32()?;
        let ef_search = r.u32()?;
        let heuristic = r.u8()? != 0;
        let seed = r.u64()?;
        let exact_threshold = r.u64()?;
        let row_count = r.u32()?;
        let live_count = r.u64()?;
        let deleted_count = r.u64()?;
        let max_seq = r.u64()?;
        let graph_entry = match r.u8()? {
            0 => None,
            1 => Some(r.u32()?),
            other => {
                return Err(VectorError::corrupt(
                    "manifest",
                    format!("graph_entry discriminant {other}"),
                ))
            }
        };
        let graph_max_level = r.u32()?;
        let vectors_checksum = r.u64()?;
        let graph_checksum = r.u64()?;
        let ids_checksum = r.u64()?;
        let meta_checksum = r.u64()?;
        let created_at_unix = r.i64()?;
        let embedding_space = EmbeddingSpace {
            provider: r.short_str()?,
            model: r.short_str()?,
            source_hash: r.short_str()?,
        };
        if r.remaining() != 0 {
            return Err(VectorError::corrupt(
                "manifest",
                format!("{} trailing bytes", r.remaining()),
            ));
        }
        Ok(Manifest {
            format_version,
            generation,
            dimensions,
            metric,
            algorithm,
            m,
            ef_construction,
            ef_search,
            heuristic,
            seed,
            exact_threshold,
            row_count,
            live_count,
            deleted_count,
            max_seq,
            graph_entry,
            graph_max_level,
            vectors_checksum,
            graph_checksum,
            ids_checksum,
            meta_checksum,
            created_at_unix,
            embedding_space,
        })
    }

    /// Verify segment checksums recorded in the manifest against the files
    /// actually on disk. Called on open, before anything is trusted.
    pub(crate) fn verify_checksums(&self, dir: &Path) -> Result<()> {
        let checks = [
            (
                segment_file("vectors", self.generation),
                self.vectors_checksum,
            ),
            (segment_file("graph", self.generation), self.graph_checksum),
            (segment_file("ids", self.generation), self.ids_checksum),
            (segment_file("meta", self.generation), self.meta_checksum),
        ];
        for (name, expected) in checks {
            let data = std::fs::read(dir.join(&name)).map_err(|error| {
                VectorError::corrupt(name.as_str(), format!("cannot read segment file: {error}"))
            })?;
            let actual = frame::checksum(&data);
            if actual != expected {
                return Err(VectorError::corrupt(
                    name.as_str(),
                    format!(
                        "segment checksum mismatch: manifest says {expected:#x}, file has {actual:#x}"
                    ),
                ));
            }
        }
        Ok(())
    }
}

/// Rebuild the runtime config recorded in a manifest (used by `open`).
pub(crate) fn config_from_manifest(manifest: &Manifest) -> IndexConfig {
    IndexConfig {
        dimensions: manifest.dimensions,
        metric: manifest.metric,
        algorithm: manifest.algorithm,
        hnsw: crate::config::HnswParams {
            m: manifest.m,
            ef_construction: manifest.ef_construction,
            ef_search: manifest.ef_search,
            heuristic: manifest.heuristic,
            seed: manifest.seed,
        },
        persistence: crate::config::PersistenceParams::default(),
        exact_threshold: manifest.exact_threshold,
        embedding_space: manifest.embedding_space.clone(),
    }
}

// ---------------------------------------------------------------------------
// ids.dat
// ---------------------------------------------------------------------------

/// ids.dat payload: row_count × 16-byte little-endian external ids.
pub(crate) fn encode_ids(ids: &[VectorId]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(ids.len() * 16);
    for id in ids {
        bytes.extend_from_slice(&id.to_le_bytes());
    }
    frame::encode(&IDS_MAGIC, frame::FORMAT_VERSION, &bytes)
}

pub(crate) fn decode_ids(data: &[u8], expected_rows: usize) -> Result<Vec<VectorId>> {
    let payload = frame::decode_v1(&IDS_MAGIC, "ids", data)?;
    if payload.len() % 16 != 0 {
        return Err(VectorError::corrupt(
            "ids",
            format!("payload length {} not a multiple of 16", payload.len()),
        ));
    }
    let ids = payload
        .chunks_exact(16)
        .map(|chunk| {
            let mut bytes = [0u8; 16];
            bytes.copy_from_slice(chunk);
            VectorId::from_le_bytes(bytes)
        })
        .collect::<Vec<_>>();
    if ids.len() != expected_rows {
        return Err(VectorError::corrupt(
            "ids",
            format!("{} ids for {} rows", ids.len(), expected_rows),
        ));
    }
    Ok(ids)
}

// ---------------------------------------------------------------------------
// meta.dat (metadata rows + alive bitmask)
// ---------------------------------------------------------------------------

/// meta.dat payload: `[per-row metadata][u32 mask_len][alive bitmask]`.
pub(crate) fn encode_meta(rows: &Rows) -> Vec<u8> {
    let mut w = Writer::with_capacity(rows.row_count() * 52 + 16);
    for m in &rows.meta {
        let flags = u8::from(m.route_id.is_some())
            | (u8::from(m.source_id.is_some()) << 1)
            | (u8::from(m.knowledge_point_id.is_some()) << 2)
            | (u8::from(m.kind.is_some()) << 3);
        w.u8(flags);
        if let Some(id) = m.route_id {
            w.bytes(&id.to_le_bytes());
        }
        if let Some(id) = m.source_id {
            w.bytes(&id.to_le_bytes());
        }
        if let Some(id) = m.knowledge_point_id {
            w.bytes(&id.to_le_bytes());
        }
        if let Some(kind) = &m.kind {
            w.short_str(kind);
        }
    }
    let mask_len = rows.alive.len().div_ceil(8);
    let mut mask = vec![0u8; mask_len];
    for (i, &alive) in rows.alive.iter().enumerate() {
        if alive {
            mask[i / 8] |= 1 << (i % 8);
        }
    }
    w.u32(mask_len as u32);
    w.bytes(&mask);
    frame::encode(&META_MAGIC, frame::FORMAT_VERSION, &w.into_bytes())
}

pub(crate) fn decode_meta(data: &[u8], expected_rows: usize) -> Result<(Vec<Metadata>, Vec<bool>)> {
    let payload = frame::decode_v1(&META_MAGIC, "meta", data)?;
    let mut r = Reader::new(payload, "meta");
    let mut meta = Vec::with_capacity(expected_rows);
    while meta.len() < expected_rows {
        if r.remaining() == 0 {
            return Err(VectorError::corrupt(
                "meta",
                format!("metadata rows ended early at {}", meta.len()),
            ));
        }
        let flags = r.u8()?;
        if flags & 0xF0 != 0 {
            return Err(VectorError::corrupt(
                "meta",
                format!("reserved flag bits set: {flags:#x}"),
            ));
        }
        let route_id = if flags & 1 != 0 {
            Some(read_id(&mut r)?)
        } else {
            None
        };
        let source_id = if flags & 2 != 0 {
            Some(read_id(&mut r)?)
        } else {
            None
        };
        let knowledge_point_id = if flags & 4 != 0 {
            Some(read_id(&mut r)?)
        } else {
            None
        };
        let kind = if flags & 8 != 0 {
            let kind = r.short_str()?;
            if kind.len() > MAX_KIND_LEN {
                return Err(VectorError::corrupt(
                    "meta",
                    format!("kind string length {} exceeds {MAX_KIND_LEN}", kind.len()),
                ));
            }
            Some(kind)
        } else {
            None
        };
        meta.push(Metadata {
            route_id,
            source_id,
            knowledge_point_id,
            kind,
        });
    }
    let mask_len = r.u32()? as usize;
    let expected_mask_bytes = expected_rows.div_ceil(8);
    if mask_len != expected_mask_bytes {
        return Err(VectorError::corrupt(
            "meta",
            format!("alive mask is {mask_len} bytes, expected {expected_mask_bytes}"),
        ));
    }
    let mask = r.raw(mask_len)?;
    if r.remaining() != 0 {
        return Err(VectorError::corrupt(
            "meta",
            format!("{} trailing bytes after alive mask", r.remaining()),
        ));
    }
    let alive = (0..expected_rows)
        .map(|i| mask[i / 8] & (1 << (i % 8)) != 0)
        .collect();
    Ok((meta, alive))
}

fn read_id(r: &mut Reader<'_>) -> Result<VectorId> {
    let bytes = r.raw(16)?;
    let mut array = [0u8; 16];
    array.copy_from_slice(bytes);
    Ok(VectorId::from_le_bytes(array))
}

// ---------------------------------------------------------------------------
// graph.dat
// ---------------------------------------------------------------------------

/// graph.dat payload: `[m][entry][max_level][node_count][levels…][links…]`.
pub(crate) fn encode_graph(graph: &HnswGraph) -> Vec<u8> {
    let mut w = Writer::with_capacity(graph.links().len() * 4 + 32);
    w.u32(graph.m());
    match graph.entry() {
        Some(entry) => {
            w.u8(1);
            w.u32(entry);
        }
        None => {
            w.u8(0);
        }
    }
    w.u32(graph.max_level());
    w.u32(graph.node_count());
    for &level in graph.levels() {
        w.u32(level);
    }
    for &link in graph.links() {
        w.u32(link);
    }
    frame::encode(&GRAPH_MAGIC, frame::FORMAT_VERSION, &w.into_bytes())
}

pub(crate) fn decode_graph(data: &[u8]) -> Result<HnswGraph> {
    let payload = frame::decode_v1(&GRAPH_MAGIC, "graph", data)?;
    let mut r = Reader::new(payload, "graph");
    let m = r.u32()?;
    let entry = match r.u8()? {
        0 => None,
        1 => Some(r.u32()?),
        other => {
            return Err(VectorError::corrupt(
                "graph",
                format!("entry discriminant {other}"),
            ))
        }
    };
    let max_level = r.u32()?;
    if max_level > MAX_LEVEL {
        return Err(VectorError::corrupt(
            "graph",
            format!("max level {max_level} exceeds {MAX_LEVEL}"),
        ));
    }
    let level_count = r.u32()? as usize;
    let mut levels = Vec::with_capacity(level_count);
    for _ in 0..level_count {
        let level = r.u32()?;
        if level > MAX_LEVEL {
            return Err(VectorError::corrupt(
                "graph",
                format!("node level {level} exceeds {MAX_LEVEL}"),
            ));
        }
        levels.push(level);
    }
    let mut links = Vec::with_capacity(r.remaining() / 4);
    while r.remaining() > 0 {
        links.push(r.u32()?);
    }
    HnswGraph::from_parts(m, levels, links, entry, max_level)
}

// ---------------------------------------------------------------------------
// Segment IO
// ---------------------------------------------------------------------------

/// Read a framed segment file from disk, validating magic/version/checksum.
pub(crate) fn read_segment(path: &Path, magic: &[u8; 4], file: &'static str) -> Result<Vec<u8>> {
    let data = std::fs::read(path).map_err(|error| {
        VectorError::corrupt(file, format!("cannot read {}: {error}", path.display()))
    })?;
    frame::decode_v1(magic, file, &data).map(|payload| payload.to_vec())
}

/// Memory-map a framed segment file and return `(mmap, payload_range)`.
/// Framing, version, and checksum are validated exactly as in [`read_segment`].
pub(crate) fn mmap_segment(
    path: &Path,
    magic: &[u8; 4],
    file: &'static str,
) -> Result<(Mmap, std::ops::Range<usize>)> {
    let file_handle = std::fs::File::open(path).map_err(|error| {
        VectorError::corrupt(file, format!("cannot open {}: {error}", path.display()))
    })?;
    // SAFETY: this is the single audited `unsafe` call in the crate (the
    // crate-level `#![forbid(unsafe_code)]` is relaxed here, and only here,
    // because mapping a file fundamentally requires it). Safety argument:
    // (1) `file_handle` was just opened and outlives the mapping — it is
    // moved into this scope and the mapping is returned while the file is
    // still open; `Mmap::map` itself keeps a duplicated handle internally in
    // memmap2 ≥ 0.9, so the mapping never outlives the file.
    // (2) The mapped file is only ever *replaced by atomic rename* during
    // checkpoints/compaction — never mutated in place — so the mapped bytes
    // cannot change while the mapping is alive.
    // (3) The mapping is read-only (`PROT_READ`); no aliasing writes.
    // (4) The payload range was validated by `frame::decode` above, so all
    // indexing into `map` below is in bounds.
    #[allow(unsafe_code)]
    let map = unsafe { Mmap::map(&file_handle) }.map_err(|error| {
        VectorError::corrupt(file, format!("cannot mmap {}: {error}", path.display()))
    })?;
    let (version, _) = frame::decode(magic, file, &map)?;
    if version != frame::FORMAT_VERSION {
        return Err(VectorError::UnsupportedVersion {
            found: version,
            supported: frame::FORMAT_VERSION,
        });
    }
    let payload_len = u64::from_le_bytes(
        map[8..16]
            .try_into()
            .expect("framing validation guarantees 16 header bytes"),
    ) as usize;
    let payload = frame::HEADER_LEN..frame::HEADER_LEN + payload_len;
    Ok((map, payload))
}

/// Checksums of the four data segments after a checkpoint/compaction write.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct SegmentChecksums {
    pub vectors: u64,
    pub graph: u64,
    pub ids: u64,
    pub meta: u64,
}

/// Write all four data segments of `generation` into `dir` (used by
/// checkpoint and compaction). Every file is staged as `.tmp`, fsynced, then
/// atomically renamed into place. The manifest is written by the caller
/// afterwards — it is the commit point of the generation.
pub(crate) fn write_segments(
    dir: &Path,
    generation: u64,
    rows: &Rows,
    graph: &HnswGraph,
    dim: usize,
) -> Result<SegmentChecksums> {
    // vectors.dat — streamed, since it can be hundreds of MB.
    let vectors_name = segment_file("vectors", generation);
    let vectors_len = (rows.row_count() * dim * 4) as u64;
    let mut vectors_writer =
        frame::StreamWriter::create(dir, &vectors_name, &VECTORS_MAGIC, vectors_len)?;
    rows.vectors
        .write_le(dim, &mut vectors_writer)
        .map_err(|error| VectorError::corrupt("vectors", format!("write failed: {error}")))?;
    let vectors_checksum = vectors_writer.finish()?;

    let ids_bytes = encode_ids(&rows.ids);
    let ids_checksum = frame::checksum(&ids_bytes);
    let meta_bytes = encode_meta(rows);
    let meta_checksum = frame::checksum(&meta_bytes);
    let graph_bytes = encode_graph(graph);
    let graph_checksum = frame::checksum(&graph_bytes);

    crate::codec::atomic_write(dir, &segment_file("ids", generation), &ids_bytes)?;
    crate::codec::atomic_write(dir, &segment_file("meta", generation), &meta_bytes)?;
    crate::codec::atomic_write(dir, &segment_file("graph", generation), &graph_bytes)?;

    Ok(SegmentChecksums {
        vectors: vectors_checksum,
        graph: graph_checksum,
        ids: ids_checksum,
        meta: meta_checksum,
    })
}

/// Load every checkpointed segment of the manifest's generation into `Rows` +
/// graph. `use_mmap` selects between the mmap-backed vectors store and a
/// heap copy.
pub(crate) fn load_segments(
    dir: &Path,
    manifest: &Manifest,
    use_mmap: bool,
) -> Result<(Rows, HnswGraph)> {
    let row_count = manifest.row_count as usize;
    let dim = manifest.dimensions as usize;
    let gen = manifest.generation;

    let ids = decode_ids(
        &std::fs::read(dir.join(segment_file("ids", gen))).map_err(|error| {
            VectorError::corrupt("ids", format!("cannot read ids segment: {error}"))
        })?,
        row_count,
    )?;
    let (meta, alive) = decode_meta(
        &std::fs::read(dir.join(segment_file("meta", gen))).map_err(|error| {
            VectorError::corrupt("meta", format!("cannot read meta segment: {error}"))
        })?,
        row_count,
    )?;
    let graph = decode_graph(
        &std::fs::read(dir.join(segment_file("graph", gen))).map_err(|error| {
            VectorError::corrupt("graph", format!("cannot read graph segment: {error}"))
        })?,
    )?;
    if graph.node_count() as usize != row_count {
        return Err(VectorError::corrupt(
            "graph",
            format!(
                "graph has {} nodes for {} rows",
                graph.node_count(),
                row_count
            ),
        ));
    }

    let vectors: VectorData = if use_mmap {
        let (map, payload) = mmap_segment(
            &dir.join(segment_file("vectors", gen)),
            &VECTORS_MAGIC,
            "vectors",
        )?;
        VectorData::from_mmap(map, payload, dim).ok_or_else(|| {
            VectorError::corrupt("vectors", "payload is not viewable as f32 floats")
        })?
    } else {
        let bytes = read_segment(
            &dir.join(segment_file("vectors", gen)),
            &VECTORS_MAGIC,
            "vectors",
        )?;
        VectorData::from_le_bytes(&bytes, dim)?
    };
    if vectors.rows(dim) != row_count {
        return Err(VectorError::corrupt(
            "vectors",
            format!(
                "vectors hold {} rows for {} ids",
                vectors.rows(dim),
                row_count
            ),
        ));
    }

    // Rebuild the idmap (live rows only) and recompute the counters — they
    // are never trusted from the manifest.
    let mut rows = Rows::empty(vectors);
    rows.ids = ids;
    rows.meta = meta;
    rows.alive = alive;
    for (row, &id) in rows.ids.iter().enumerate() {
        if rows.alive[row] {
            rows.idmap.insert(id, row as u32);
            rows.live_count += 1;
        } else {
            rows.deleted_count += 1;
        }
    }
    Ok((rows, graph))
}

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

pub(crate) fn manifest_path(dir: &Path) -> PathBuf {
    dir.join("manifest.bin")
}

pub(crate) fn wal_path(dir: &Path) -> PathBuf {
    dir.join("wal.log")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_manifest() -> Manifest {
        Manifest {
            format_version: 1,
            generation: 3,
            dimensions: 768,
            metric: Metric::Cosine,
            algorithm: Algorithm::Adaptive,
            m: 16,
            ef_construction: 200,
            ef_search: 128,
            heuristic: true,
            seed: 42,
            exact_threshold: 4096,
            row_count: 1234,
            live_count: 1000,
            deleted_count: 234,
            max_seq: 1234,
            graph_entry: Some(17),
            graph_max_level: 3,
            vectors_checksum: 1,
            graph_checksum: 2,
            ids_checksum: 3,
            meta_checksum: 4,
            created_at_unix: 1_700_000_000,
            embedding_space: EmbeddingSpace {
                provider: "verruckt".into(),
                model: "bge-m3".into(),
                source_hash: "abc".into(),
            },
        }
    }

    #[test]
    fn manifest_roundtrip() {
        let manifest = sample_manifest();
        let decoded = Manifest::decode(&manifest.encode()).unwrap();
        assert_eq!(decoded, manifest);
    }

    #[test]
    fn manifest_rejects_bad_magic() {
        let mut bytes = sample_manifest().encode();
        bytes[0] ^= 0xFF; // break magic
        assert!(matches!(
            Manifest::decode(&bytes),
            Err(VectorError::CorruptIndex { .. })
        ));
    }

    #[test]
    fn manifest_rejects_newer_version() {
        let mut bytes = sample_manifest().encode();
        // Bump the inner format_version field (u32 at payload offset 0).
        bytes[frame::HEADER_LEN] = 2;
        bytes[frame::HEADER_LEN + 1] = 0;
        bytes[frame::HEADER_LEN + 2] = 0;
        bytes[frame::HEADER_LEN + 3] = 0;
        // Re-checksum so only the version is wrong, nothing else.
        let body = &bytes[..bytes.len() - frame::TRAILER_LEN];
        let checksum = crate::codec::fnv1a64(body);
        bytes.truncate(body.len());
        bytes.extend_from_slice(&checksum.to_le_bytes());
        match Manifest::decode(&bytes) {
            Err(VectorError::UnsupportedVersion { found, supported }) => {
                assert_eq!((found, supported), (2, 1));
            }
            other => panic!("expected UnsupportedVersion, got {other:?}"),
        }
    }

    #[test]
    fn graph_roundtrip_empty() {
        let graph = HnswGraph::new(4);
        let bytes = encode_graph(&graph);
        let decoded = decode_graph(&bytes).unwrap();
        assert_eq!(decoded.node_count(), 0);
        assert!(decoded.entry().is_none());
    }

    #[test]
    fn cleanup_keeps_current_generation() {
        let dir = tempfile::tempdir().unwrap();
        for (stem, gen) in [
            ("vectors", 1u64),
            ("vectors", 2),
            ("graph", 2),
            ("ids", 2),
            ("meta", 2),
        ] {
            std::fs::write(dir.path().join(segment_file(stem, gen)), b"x").unwrap();
        }
        std::fs::write(dir.path().join("vectors.2.dat.tmp"), b"x").unwrap();
        cleanup_stale_segments(dir.path(), 2).unwrap();
        assert!(dir.path().join(segment_file("vectors", 2)).exists());
        assert!(!dir.path().join(segment_file("vectors", 1)).exists());
        assert!(!dir.path().join("vectors.2.dat.tmp").exists());
        assert_eq!(directory_size(dir.path()), 4); // four 1-byte gen-2 files
    }
}
