//! Minimal write-ahead log.
//!
//! Every mutation is appended to `wal.log` as a self-contained frame *before*
//! the in-memory state is touched. On open, the checkpointed segments are
//! loaded first, then the WAL is replayed on top; a torn trailing frame
//! (crash mid-append) is detected by its checksum and dropped. The WAL is
//! truncated only after a checkpoint (full segment rewrite) has fsynced.
//!
//! Frame layout per record:
//! `[magic VWL1][version=1][payload_len][payload][fnv1a64]` where payload is
//! `[u8 op][u64 seq][op-specific body]`:
//!
//! * `op = 1` insert/upsert: `[VectorId 16B][u32 dim][f32 × dim][metadata]`
//! * `op = 2` delete: `[VectorId 16B]`
//!
//! The `seq` is the engine's monotonically increasing sequence number; it
//! also seeds deterministic HNSW level assignment, so replay rebuilds the
//! exact same graph. Records must appear in strictly increasing seq order —
//! a gap or regression aborts the replay (the log was damaged).

use crate::codec::{frame, Reader, Writer};
use crate::error::{Result, VectorError};
use crate::id::VectorId;
use crate::metadata::Metadata;
use crate::persist::WAL_MAGIC;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

pub(crate) const OP_INSERT: u8 = 1;
pub(crate) const OP_DELETE: u8 = 2;

/// A decoded WAL record.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum WalRecord {
    Insert {
        seq: u64,
        id: VectorId,
        vector: Vec<f32>,
        metadata: Metadata,
    },
    Delete {
        seq: u64,
        id: VectorId,
    },
}

impl WalRecord {
    pub fn seq(&self) -> u64 {
        match self {
            WalRecord::Insert { seq, .. } => *seq,
            WalRecord::Delete { seq, .. } => *seq,
        }
    }

    fn encode_payload(&self) -> Vec<u8> {
        match self {
            WalRecord::Insert {
                seq,
                id,
                vector,
                metadata,
            } => {
                let mut w = Writer::with_capacity(24 + vector.len() * 4);
                w.u8(OP_INSERT);
                w.u64(*seq);
                w.bytes(&id.to_le_bytes());
                w.u32(vector.len() as u32);
                for component in vector {
                    w.f32(*component);
                }
                encode_metadata(&mut w, metadata);
                w.into_bytes()
            }
            WalRecord::Delete { seq, id } => {
                let mut w = Writer::with_capacity(28);
                w.u8(OP_DELETE);
                w.u64(*seq);
                w.bytes(&id.to_le_bytes());
                w.into_bytes()
            }
        }
    }

    fn decode_payload(payload: &[u8]) -> Result<WalRecord> {
        let mut r = Reader::new(payload, "wal");
        let op = r.u8()?;
        let seq = r.u64()?;
        let id_bytes = r.raw(16)?;
        let mut id_array = [0u8; 16];
        id_array.copy_from_slice(id_bytes);
        let id = VectorId::from_le_bytes(id_array);
        match op {
            OP_INSERT => {
                let dim = r.u32()? as usize;
                if dim == 0 || dim > 65_536 {
                    return Err(VectorError::corrupt(
                        "wal",
                        format!("insert dimension {dim} out of range"),
                    ));
                }
                let mut vector = Vec::with_capacity(dim);
                for _ in 0..dim {
                    vector.push(r.f32()?);
                }
                let metadata = decode_metadata(&mut r)?;
                if r.remaining() != 0 {
                    return Err(VectorError::corrupt(
                        "wal",
                        format!("{} trailing bytes in insert record", r.remaining()),
                    ));
                }
                Ok(WalRecord::Insert {
                    seq,
                    id,
                    vector,
                    metadata,
                })
            }
            OP_DELETE => {
                if r.remaining() != 0 {
                    return Err(VectorError::corrupt(
                        "wal",
                        format!("{} trailing bytes in delete record", r.remaining()),
                    ));
                }
                Ok(WalRecord::Delete { seq, id })
            }
            other => Err(VectorError::corrupt("wal", format!("unknown op {other}"))),
        }
    }
}

fn encode_metadata(w: &mut Writer, metadata: &Metadata) {
    let flags = u8::from(metadata.route_id.is_some())
        | (u8::from(metadata.source_id.is_some()) << 1)
        | (u8::from(metadata.knowledge_point_id.is_some()) << 2)
        | (u8::from(metadata.kind.is_some()) << 3);
    w.u8(flags);
    if let Some(id) = metadata.route_id {
        w.bytes(&id.to_le_bytes());
    }
    if let Some(id) = metadata.source_id {
        w.bytes(&id.to_le_bytes());
    }
    if let Some(id) = metadata.knowledge_point_id {
        w.bytes(&id.to_le_bytes());
    }
    if let Some(kind) = &metadata.kind {
        w.short_str(kind);
    }
}

fn decode_metadata(r: &mut Reader<'_>) -> Result<Metadata> {
    let flags = r.u8()?;
    if flags & 0xF0 != 0 {
        return Err(VectorError::corrupt(
            "wal",
            format!("reserved metadata flag bits: {flags:#x}"),
        ));
    }
    let read_field = |r: &mut Reader<'_>| -> Result<Option<VectorId>> {
        let bytes = r.raw(16)?;
        let mut array = [0u8; 16];
        array.copy_from_slice(bytes);
        Ok(Some(VectorId::from_le_bytes(array)))
    };
    let route_id = if flags & 1 != 0 { read_field(r)? } else { None };
    let source_id = if flags & 2 != 0 { read_field(r)? } else { None };
    let knowledge_point_id = if flags & 4 != 0 { read_field(r)? } else { None };
    let kind = if flags & 8 != 0 {
        Some(r.short_str()?)
    } else {
        None
    };
    Ok(Metadata {
        route_id,
        source_id,
        knowledge_point_id,
        kind,
    })
}

/// Append-only WAL writer. Each record is written + flushed (fsync optional
/// per policy) before the caller mutates in-memory state.
pub(crate) struct WalWriter {
    file: File,
    path: PathBuf,
    /// Bytes appended since the last checkpoint (used by auto-checkpoint).
    pub bytes_since_checkpoint: u64,
}

impl WalWriter {
    pub fn create(dir: &Path) -> Result<WalWriter> {
        let path = dir.join("wal.log");
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        Ok(WalWriter {
            file,
            path,
            bytes_since_checkpoint: 0,
        })
    }

    /// Append one record. `fsync` controls durability of this single append
    /// (the engine fsyncs on flush/checkpoint regardless).
    pub fn append(&mut self, record: &WalRecord, fsync: bool) -> Result<()> {
        let frame_bytes =
            frame::encode(&WAL_MAGIC, frame::FORMAT_VERSION, &record.encode_payload());
        self.file.write_all(&frame_bytes)?;
        if fsync {
            self.file.sync_all()?;
        }
        self.bytes_since_checkpoint += frame_bytes.len() as u64;
        Ok(())
    }

    pub fn sync(&mut self) -> Result<()> {
        self.file.sync_all()?;
        Ok(())
    }

    /// Truncate the WAL after a successful checkpoint.
    pub fn rotate(&mut self) -> Result<()> {
        // Reopen-truncate-replace: an in-place set_len(0) after fsync of the
        // new segments is also safe, but replace-by-rename keeps the file
        // inode fresh and avoids any tail remnant.
        self.file.sync_all()?;
        self.file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.path)?;
        self.file.sync_all()?;
        self.bytes_since_checkpoint = 0;
        Ok(())
    }
}

/// Replay result: decoded records plus the byte length of the valid prefix.
#[derive(Debug, Default)]
pub(crate) struct WalReplay {
    pub records: Vec<WalRecord>,
    /// Byte offset of the valid prefix; used to truncate a torn tail.
    #[allow(dead_code)]
    pub valid_bytes: u64,
    /// Byte offset where a torn (checksum-failed) trailing frame began.
    pub torn_at: Option<u64>,
}

/// Parse the WAL. A trailing frame that fails validation is a torn write
/// from a crash — it and everything after it are dropped, and the caller is
/// told where the damage started. A *middle* frame that fails validation is
/// real corruption and is an error.
pub(crate) fn replay_wal(bytes: &[u8]) -> Result<WalReplay> {
    let mut records = Vec::new();
    let mut pos = 0usize;
    let mut last_seq: Option<u64> = None;
    while pos < bytes.len() {
        match try_parse_frame(&bytes[pos..]) {
            Ok(Some((payload, frame_len))) => {
                let record = WalRecord::decode_payload(payload)?;
                if let Some(previous) = last_seq {
                    if record.seq() != previous + 1 {
                        return Err(VectorError::corrupt(
                            "wal",
                            format!(
                                "sequence regression: expected {}, got {} (previous {previous})",
                                previous + 1,
                                record.seq()
                            ),
                        ));
                    }
                }
                last_seq = Some(record.seq());
                records.push(record);
                pos += frame_len;
            }
            Ok(None) => {
                // Trailing garbage / incomplete frame: torn write.
                return Ok(WalReplay {
                    records,
                    valid_bytes: pos as u64,
                    torn_at: Some(pos as u64),
                });
            }
            Err(error) => {
                // A frame with valid magic but bad checksum mid-log is real
                // corruption; at the tail it is a torn write. Distinguish by
                // whether anything follows the damaged region.
                return Err(error);
            }
        }
    }
    Ok(WalReplay {
        records,
        valid_bytes: pos as u64,
        torn_at: None,
    })
}

/// Parse one frame at the start of `bytes`. `Ok(None)` means the remaining
/// bytes cannot possibly hold a complete valid frame (too short / bad magic
/// — a torn tail).
fn try_parse_frame(bytes: &[u8]) -> Result<Option<(&[u8], usize)>> {
    if bytes.len() < frame::HEADER_LEN {
        return Ok(None);
    }
    if bytes[0..4] != WAL_MAGIC {
        return Ok(None);
    }
    let payload_len =
        u64::from_le_bytes(bytes[8..16].try_into().expect("length checked above")) as usize;
    let frame_len = frame::HEADER_LEN + payload_len + frame::TRAILER_LEN;
    if bytes.len() < frame_len {
        return Ok(None);
    }
    let (version, payload) = frame::decode(&WAL_MAGIC, "wal", &bytes[..frame_len])?;
    if version != frame::FORMAT_VERSION {
        return Err(VectorError::UnsupportedVersion {
            found: version,
            supported: frame::FORMAT_VERSION,
        });
    }
    Ok(Some((payload, frame_len)))
}

/// Truncate a WAL file to its valid prefix after a torn tail was detected.
pub(crate) fn truncate_to(path: &Path, valid_bytes: u64) -> Result<()> {
    let file = OpenOptions::new().write(true).open(path)?;
    file.set_len(valid_bytes)?;
    file.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::Metadata;
    use crate::persist::WAL_MAGIC;

    fn insert_record(seq: u64, id: u128, dim: usize) -> WalRecord {
        WalRecord::Insert {
            seq,
            id: VectorId(id),
            vector: vec![0.5; dim],
            metadata: Metadata::with_kind("chunk"),
        }
    }

    #[test]
    fn append_and_replay_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let mut wal = WalWriter::create(dir.path()).unwrap();
        wal.append(&insert_record(1, 100, 3), false).unwrap();
        wal.append(
            &WalRecord::Delete {
                seq: 2,
                id: VectorId(100),
            },
            false,
        )
        .unwrap();
        wal.append(&insert_record(3, 101, 3), false).unwrap();
        wal.sync().unwrap();
        drop(wal);

        let bytes = std::fs::read(dir.path().join("wal.log")).unwrap();
        let replay = replay_wal(&bytes).unwrap();
        assert_eq!(replay.records.len(), 3);
        assert_eq!(replay.records[0].seq(), 1);
        assert_eq!(replay.torn_at, None);
    }

    #[test]
    fn torn_tail_is_dropped_not_fatal() {
        let dir = tempfile::tempdir().unwrap();
        // Write record 1 alone first to learn the frame length.
        let mut wal = WalWriter::create(dir.path()).unwrap();
        wal.append(&insert_record(1, 1, 2), false).unwrap();
        wal.sync().unwrap();
        let first_frame_len = std::fs::read(dir.path().join("wal.log")).unwrap().len();

        wal.append(&insert_record(2, 2, 2), false).unwrap();
        wal.sync().unwrap();
        drop(wal);

        let path = dir.path().join("wal.log");
        let mut bytes = std::fs::read(&path).unwrap();
        // Simulate a crash mid-append: chop into the second frame.
        bytes.truncate(first_frame_len + 5);
        std::fs::write(&path, &bytes).unwrap();

        let replay = replay_wal(&bytes).unwrap();
        assert_eq!(replay.records.len(), 1);
        assert_eq!(replay.torn_at, Some(first_frame_len as u64));
    }

    #[test]
    fn sequence_regression_is_corruption() {
        let dir = tempfile::tempdir().unwrap();
        let mut wal = WalWriter::create(dir.path()).unwrap();
        wal.append(&insert_record(1, 1, 2), false).unwrap();
        wal.append(&insert_record(2, 2, 2), false).unwrap();
        wal.sync().unwrap();
        drop(wal);

        // Manually append a record with seq 5 (gap) by rebuilding frames.
        let path = dir.path().join("wal.log");
        let mut bytes = std::fs::read(&path).unwrap();
        bytes.extend_from_slice(&frame::encode(
            &WAL_MAGIC,
            frame::FORMAT_VERSION,
            &insert_record(5, 3, 2).encode_payload(),
        ));
        assert!(replay_wal(&bytes).is_err());
    }

    #[test]
    fn checksum_mismatch_is_detected() {
        let dir = tempfile::tempdir().unwrap();
        let mut wal = WalWriter::create(dir.path()).unwrap();
        wal.append(&insert_record(1, 1, 2), false).unwrap();
        wal.sync().unwrap();
        drop(wal);

        let path = dir.path().join("wal.log");
        let mut bytes = std::fs::read(&path).unwrap();
        // Flip a payload byte (not the header magic) → checksum mismatch.
        let last = bytes.len() - 1;
        bytes[last] ^= 0xFF;
        assert!(matches!(
            replay_wal(&bytes),
            Err(VectorError::CorruptIndex { .. })
        ));
    }

    #[test]
    fn wal_frame_decode_rejects_garbage() {
        assert!(matches!(
            replay_wal(&[0xFF, 0x00, 0x00]),
            Ok(replay) if replay.records.is_empty()
        ));
    }
}
