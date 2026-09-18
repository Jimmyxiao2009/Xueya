//! In-memory row storage.
//!
//! Vectors live in one flat, append-only `f32` arena — row `i` occupies
//! `data[i * dim .. (i + 1) * dim]` — never in a `Vec<Vec<f32>>`, so exact
//! search is a linear scan over contiguous memory (auto-vectorizable, no
//! pointer chasing).
//!
//! Two backing modes:
//!
//! * `Heap` — a plain `Vec<f32>`; every open copies the checkpointed vectors
//!   into the heap.
//! * `Mmap` — the checkpointed prefix is memory-mapped directly from the
//!   vectors segment (zero-copy, lazily paged by the OS → fast startup, low
//!   heap), while rows appended after the last checkpoint live in a heap
//!   tail. Rows are never mutated in place — upserts tombstone the old row
//!   and append a new one — so the mapping stays valid for the engine's
//!   lifetime and can be swapped at the next checkpoint.

use crate::error::{Result, VectorError};
use crate::id::VectorId;
use crate::metadata::Metadata;
use memmap2::Mmap;
use std::collections::HashMap;

/// Raw vector matrix, append-only.
pub(crate) enum VectorData {
    Heap { data: Vec<f32> },
    Mmap {
        map: Mmap,
        /// Byte range of the frame payload inside `map`.
        payload: std::ops::Range<usize>,
        base_rows: usize,
        tail: Vec<f32>,
    },
}

impl VectorData {
    pub fn empty_heap() -> VectorData {
        VectorData::Heap { data: Vec::new() }
    }

    pub fn rows(&self, dim: usize) -> usize {
        match self {
            VectorData::Heap { data } => data.len() / dim,
            VectorData::Mmap {
                base_rows, tail, ..
            } => base_rows + tail.len() / dim,
        }
    }

    /// The row's slice into the flat arena. `index` must be < `rows(dim)`.
    pub fn row(&self, index: usize, dim: usize) -> &[f32] {
        match self {
            VectorData::Heap { data } => &data[index * dim..(index + 1) * dim],
            VectorData::Mmap {
                map,
                payload,
                base_rows,
                tail,
            } => {
                if index < *base_rows {
                    // Invariant (established by `from_mmap`): payload length is
                    // a multiple of 4 and the range starts 16 bytes into a
                    // page-aligned mapping, so the cast below always succeeds.
                    let floats = bytemuck::cast_slice::<u8, f32>(&map[payload.clone()]);
                    &floats[index * dim..(index + 1) * dim]
                } else {
                    let tail_index = index - base_rows;
                    &tail[tail_index * dim..(tail_index + 1) * dim]
                }
            }
        }
    }

    pub fn push_row(&mut self, row: &[f32]) {
        match self {
            VectorData::Heap { data } => data.extend_from_slice(row),
            VectorData::Mmap { tail, .. } => tail.extend_from_slice(row),
        }
    }

    /// Heap bytes owned by this store (excludes the mmap'd prefix; those
    /// bytes are file-backed page cache, not engine heap).
    pub fn heap_bytes(&self) -> u64 {
        match self {
            VectorData::Heap { data } => (data.len() * 4) as u64,
            VectorData::Mmap { tail, .. } => (tail.len() * 4) as u64,
        }
    }

    /// Stream every row to `out` as little-endian f32 bytes, updating the
    /// running checksum. Used by checkpointing.
    pub fn write_le<W: std::io::Write>(
        &self,
        dim: usize,
        out: &mut W,
    ) -> std::io::Result<()> {
        const CHUNK: usize = 4096;
        match self {
            VectorData::Heap { data } => {
                let mut buf = Vec::with_capacity(CHUNK * 4);
                for chunk in data.chunks(CHUNK) {
                    write_f32_chunk_le(chunk, &mut buf);
                    out.write_all(&buf)?;
                    buf.clear();
                }
            }
            VectorData::Mmap {
                map, payload, tail, ..
            } => {
                // The mmap'd bytes are already the on-disk little-endian form.
                out.write_all(&map[payload.clone()])?;
                let mut buf = Vec::with_capacity(CHUNK * 4);
                for chunk in tail.chunks(CHUNK) {
                    write_f32_chunk_le(chunk, &mut buf);
                    out.write_all(&buf)?;
                    buf.clear();
                }
            }
        }
        let _ = dim;
        Ok(())
    }

    /// Build the mmap-backed variant from a validated frame payload.
    /// Returns `None` when the payload cannot be viewed as `f32`s (length not
    /// divisible by 4, or misalignment) — callers then fall back to a heap copy.
    pub fn from_mmap(map: Mmap, payload: std::ops::Range<usize>, dim: usize) -> Option<VectorData> {
        let bytes = &map[payload.clone()];
        if bytes.len() % 4 != 0 || bytemuck::try_cast_slice::<u8, f32>(bytes).is_err() {
            return None;
        }
        let base_rows = bytes.len() / 4 / dim;
        Some(VectorData::Mmap {
            map,
            payload,
            base_rows,
            tail: Vec::new(),
        })
    }

    /// Copy a validated frame payload into a heap store.
    /// On big-endian hosts the little-endian bytes are converted component by
    /// component; on little-endian hosts this is a plain memcpy.
    pub fn from_le_bytes(bytes: &[u8], dim: usize) -> Result<VectorData> {
        if bytes.len() % 4 != 0 {
            return Err(VectorError::corrupt(
                "vectors",
                format!("payload length {} is not a multiple of 4", bytes.len()),
            ));
        }
        #[cfg(target_endian = "little")]
        {
            let data: Vec<f32> = bytemuck::cast_slice::<u8, f32>(bytes).to_vec();
            let _ = dim;
            Ok(VectorData::Heap { data })
        }
        #[cfg(target_endian = "big")]
        {
            let mut data = Vec::with_capacity(bytes.len() / 4);
            for chunk in bytes.chunks_exact(4) {
                data.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
            }
            let _ = dim;
            Ok(VectorData::Heap { data })
        }
    }
}

fn write_f32_chunk_le(chunk: &[f32], buf: &mut Vec<u8>) {
    #[cfg(target_endian = "little")]
    {
        buf.extend_from_slice(bytemuck::cast_slice::<f32, u8>(chunk));
    }
    #[cfg(target_endian = "big")]
    {
        for value in chunk {
            buf.extend_from_slice(&value.to_le_bytes());
        }
    }
}

/// All per-row state plus the external-id ↔ internal-row mapping.
pub(crate) struct Rows {
    pub ids: Vec<VectorId>,
    pub meta: Vec<Metadata>,
    pub alive: Vec<bool>,
    pub idmap: HashMap<VectorId, u32>,
    pub vectors: VectorData,
    pub live_count: u64,
    pub deleted_count: u64,
}

impl Rows {
    pub fn empty(vectors: VectorData) -> Rows {
        Rows {
            ids: Vec::new(),
            meta: Vec::new(),
            alive: Vec::new(),
            idmap: HashMap::new(),
            vectors,
            live_count: 0,
            deleted_count: 0,
        }
    }

    pub fn row_count(&self) -> usize {
        self.ids.len()
    }

    /// Insert (or upsert: an existing live row with the same external id is
    /// tombstoned first). Returns the new row index.
    pub fn insert(&mut self, id: VectorId, vector: &[f32], metadata: Metadata) -> u32 {
        if let Some(&old_row) = self.idmap.get(&id) {
            self.tombstone(old_row);
        }
        let row = self.ids.len() as u32;
        self.ids.push(id);
        self.meta.push(metadata);
        self.alive.push(true);
        self.vectors.push_row(vector);
        self.idmap.insert(id, row);
        self.live_count += 1;
        row
    }

    /// Tombstone a row and drop its id-mapping entry if it points here.
    pub fn tombstone(&mut self, row: u32) {
        let row_idx = row as usize;
        if !self.alive[row_idx] {
            return;
        }
        self.alive[row_idx] = false;
        self.live_count -= 1;
        self.deleted_count += 1;
        let id = self.ids[row_idx];
        if self.idmap.get(&id) == Some(&row) {
            self.idmap.remove(&id);
        }
    }

    /// Copy an existing row's vector (used by compaction).
    pub fn copy_vector(&self, row: u32, dim: usize) -> Vec<f32> {
        self.vectors.row(row as usize, dim).to_vec()
    }

    pub fn heap_bytes(&self) -> u64 {
        self.vectors.heap_bytes()
            + (self.ids.len() * 16) as u64
            + (self.alive.len()) as u64
            + (self.idmap.len() * 24) as u64
    }
}
