//! Explicit little-endian binary codec + FNV-1a-64 checksums.
//!
//! The on-disk format is written field-by-field in little-endian byte order
//! (see docs/FORMAT.md). Nothing is ever memory-dumped from Rust structs, so
//! there are no padding / ABI portability concerns.

use crate::error::{Result, VectorError};

/// Incremental FNV-1a 64-bit (used to checksum large payloads while streaming
/// them to disk, so checkpointing never needs a second pass over the data).
#[derive(Debug, Clone)]
pub struct Fnv1a64 {
    hash: u64,
}

impl Fnv1a64 {
    pub fn new() -> Self {
        Fnv1a64 {
            hash: 0xcbf2_9ce4_8422_2325,
        }
    }

    pub fn update(&mut self, bytes: &[u8]) {
        const PRIME: u64 = 0x0000_0100_0000_01b3;
        for &byte in bytes {
            self.hash ^= byte as u64;
            self.hash = self.hash.wrapping_mul(PRIME);
        }
    }

    pub fn finish(&self) -> u64 {
        self.hash
    }
}

impl Default for Fnv1a64 {
    fn default() -> Self {
        Fnv1a64::new()
    }
}

/// FNV-1a 64-bit checksum. Cheap, portable, and only used to *detect*
/// corruption (not to protect against adversaries).
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hasher = Fnv1a64::new();
    hasher.update(bytes);
    hasher.finish()
}

/// Append-only little-endian writer over a `Vec<u8>` buffer.
#[derive(Debug, Default)]
pub struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Writer { buf: Vec::new() }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Writer {
            buf: Vec::with_capacity(capacity),
        }
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    pub fn u8(&mut self, value: u8) -> &mut Self {
        self.buf.push(value);
        self
    }

    pub fn u16(&mut self, value: u16) -> &mut Self {
        self.buf.extend_from_slice(&value.to_le_bytes());
        self
    }

    pub fn u32(&mut self, value: u32) -> &mut Self {
        self.buf.extend_from_slice(&value.to_le_bytes());
        self
    }

    pub fn u64(&mut self, value: u64) -> &mut Self {
        self.buf.extend_from_slice(&value.to_le_bytes());
        self
    }

    pub fn i64(&mut self, value: i64) -> &mut Self {
        self.buf.extend_from_slice(&value.to_le_bytes());
        self
    }

    pub fn f32(&mut self, value: f32) -> &mut Self {
        self.buf.extend_from_slice(&value.to_le_bytes());
        self
    }

    pub fn bytes(&mut self, value: &[u8]) -> &mut Self {
        self.buf.extend_from_slice(value);
        self
    }

    /// Length-prefixed (u16) byte string; bounded fields only.
    pub fn short_str(&mut self, value: &str) -> &mut Self {
        debug_assert!(value.len() <= u16::MAX as usize);
        self.u16(value.len() as u16);
        self.bytes(value.as_bytes());
        self
    }

    /// Copy of the bytes written so far (used to checksum framed payloads).
    pub fn snapshot(&self) -> Vec<u8> {
        self.buf.clone()
    }
}

/// Cursor-based little-endian reader.
pub struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
    file: &'static str,
}

impl<'a> Reader<'a> {
    pub fn new(buf: &'a [u8], file: &'static str) -> Self {
        Reader { buf, pos: 0, file }
    }

    pub fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }

    #[allow(dead_code)]
    pub fn position(&self) -> usize {
        self.pos
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        if self.remaining() < len {
            return Err(VectorError::corrupt(
                self.file,
                format!(
                    "unexpected end of data at offset {} (needed {} bytes, had {})",
                    self.pos,
                    len,
                    self.remaining()
                ),
            ));
        }
        let slice = &self.buf[self.pos..self.pos + len];
        self.pos += len;
        Ok(slice)
    }

    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    pub fn u16(&mut self) -> Result<u16> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    pub fn u32(&mut self) -> Result<u32> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    pub fn u64(&mut self) -> Result<u64> {
        let bytes = self.take(8)?;
        let mut array = [0u8; 8];
        array.copy_from_slice(bytes);
        Ok(u64::from_le_bytes(array))
    }

    pub fn i64(&mut self) -> Result<i64> {
        let bytes = self.take(8)?;
        let mut array = [0u8; 8];
        array.copy_from_slice(bytes);
        Ok(i64::from_le_bytes(array))
    }

    pub fn f32(&mut self) -> Result<f32> {
        let bytes = self.take(4)?;
        Ok(f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    pub fn magic(&mut self, expected: &[u8; 4]) -> Result<()> {
        let bytes = self.take(4)?;
        if bytes != expected {
            return Err(VectorError::corrupt(
                self.file,
                format!(
                    "bad magic {:02x?}, expected {:02x?}",
                    bytes, expected
                ),
            ));
        }
        Ok(())
    }

    pub fn short_str(&mut self) -> Result<String> {
        let len = self.u16()? as usize;
        let bytes = self.take(len)?;
        String::from_utf8(bytes.to_vec())
            .map_err(|_| VectorError::corrupt(self.file, "invalid utf-8 in short string"))
    }

    pub fn raw(&mut self, len: usize) -> Result<&'a [u8]> {
        self.take(len)
    }
}

/// Framing for checkpoint data files: `[magic][version][payload_len][payload][checksum]`.
///
/// `checksum` is FNV-1a-64 over everything before it, so truncation and bit
/// flips are always detected on open.
pub mod frame {
    use super::{fnv1a64, Fnv1a64, Reader, Writer};
    use crate::error::{Result, VectorError};
    use std::io::Write;
    use std::path::Path;

    pub const TRAILER_LEN: usize = 8;
    /// The one and only on-disk format version this build reads/writes.
    pub const FORMAT_VERSION: u32 = 1;
    /// Header length: magic(4) + version(4) + payload_len(8).
    pub const HEADER_LEN: usize = 16;

    /// Write `payload` framed with the given magic and version.
    /// Returns the framed bytes and the checksum (which is also what the
    /// manifest records for this file).
    pub fn encode(magic: &[u8; 4], version: u32, payload: &[u8]) -> Vec<u8> {
        let mut writer = Writer::with_capacity(payload.len() + HEADER_LEN + TRAILER_LEN);
        writer.bytes(magic);
        writer.u32(version);
        writer.u64(payload.len() as u64);
        writer.bytes(payload);
        let checksum = fnv1a64(&writer.snapshot());
        writer.u64(checksum);
        writer.into_bytes()
    }

    /// Validate framing, return `(version, payload)`.
    pub fn decode<'a>(
        magic: &[u8; 4],
        file: &'static str,
        data: &'a [u8],
    ) -> Result<(u32, &'a [u8])> {
        let mut reader = Reader::new(data, file);
        reader.magic(magic)?;
        let version = reader.u32()?;
        let payload_len = reader.u64()? as usize;
        let payload = reader.raw(payload_len)?;
        let checksum_bytes = reader.raw(TRAILER_LEN)?;
        if reader.remaining() != 0 {
            return Err(VectorError::corrupt(
                file,
                format!("{} trailing bytes after checksum", reader.remaining()),
            ));
        }
        let mut checksum_array = [0u8; 8];
        checksum_array.copy_from_slice(checksum_bytes);
        let checksum = u64::from_le_bytes(checksum_array);
        let header_and_payload = &data[..data.len() - TRAILER_LEN];
        if fnv1a64(header_and_payload) != checksum {
            return Err(VectorError::corrupt(file, "checksum mismatch"));
        }
        Ok((version, payload))
    }

    /// [`decode`] plus the format-version gate: any version other than
    /// [`FORMAT_VERSION`] is reported as `UnsupportedVersion`, never guessed.
    pub fn decode_v1<'a>(
        magic: &[u8; 4],
        file: &'static str,
        data: &'a [u8],
    ) -> Result<&'a [u8]> {
        let (version, payload) = decode(magic, file, data)?;
        if version != FORMAT_VERSION {
            return Err(VectorError::UnsupportedVersion {
                found: version,
                supported: FORMAT_VERSION,
            });
        }
        Ok(payload)
    }

    /// The checksum of an already-validated framed file (its last 8 bytes).
    /// Must only be called on data that passed [`decode`].
    pub fn checksum(data: &[u8]) -> u64 {
        let mut array = [0u8; 8];
        array.copy_from_slice(&data[data.len() - TRAILER_LEN..]);
        u64::from_le_bytes(array)
    }

    /// Streaming framed writer for payloads too large to buffer in memory
    /// (the vectors segment). Writes the header up front, checksums each
    /// chunk as it is written, and finishes with the checksum trailer, an
    /// fsync, and the atomic tmp→final rename.
    pub struct StreamWriter {
        file: std::fs::File,
        hasher: Fnv1a64,
        dir: std::path::PathBuf,
        name: String,
    }

    impl StreamWriter {
        pub fn create(
            dir: &Path,
            name: &str,
            magic: &[u8; 4],
            payload_len: u64,
        ) -> Result<StreamWriter> {
            let mut file = std::fs::File::create(dir.join(format!("{name}.tmp")))?;
            let mut hasher = Fnv1a64::new();
            let mut header = Writer::with_capacity(HEADER_LEN);
            header.bytes(magic);
            header.u32(FORMAT_VERSION);
            header.u64(payload_len);
            let header_bytes = header.into_bytes();
            file.write_all(&header_bytes)?;
            hasher.update(&header_bytes);
            Ok(StreamWriter {
                file,
                hasher,
                dir: dir.to_path_buf(),
                name: name.to_string(),
            })
        }

        #[allow(dead_code)]
        pub fn write_chunk(&mut self, chunk: &[u8]) -> Result<()> {
            self.file.write_all(chunk)?;
            self.hasher.update(chunk);
            Ok(())
        }

    /// Finish the file: trailer, fsync, atomic rename. Returns the frame
    /// checksum (recorded in the manifest and verified on open).
    pub fn finish(self) -> Result<u64> {
        let mut file = self.file;
        let checksum = self.hasher.finish();
        file.write_all(&checksum.to_le_bytes())?;
        file.sync_all()?;
        drop(file);
        let tmp = self.dir.join(format!("{}.tmp", self.name));
        let final_path = self.dir.join(&self.name);
        std::fs::rename(&tmp, &final_path)?;
        super::fsync_dir(&self.dir)?;
        Ok(checksum)
    }
}

impl std::io::Write for StreamWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.file.write_all(buf)?;
        self.hasher.update(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}
}

/// Best-effort directory fsync.
/// On Unix this fsyncs the directory so renames are durable. Windows does not
/// support opening directories through `std::fs::File`; there, `fs::rename`
/// (`MoveFileEx` with `MOVEFILE_REPLACE_EXISTING`) provides the atomicity we
/// rely on and directory fsync is skipped.
pub fn fsync_dir(path: &std::path::Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        // Read-only open of a directory works on Linux/macOS and lets us
        // fsync the directory entry itself, making renames durable.
        let file = std::fs::File::open(path)?;
        file.sync_all()?;
    }
    #[cfg(not(unix))]
    {
        // Windows does not support opening directories through std::fs::File;
        // there, `fs::rename` (MoveFileEx + REPLACE_EXISTING) is the atomicity
        // barrier the commit protocol relies on.
        let _ = path;
    }
    Ok(())
}

/// Write a file atomically: write to `name.tmp`, fsync, rename over `name`.
pub fn atomic_write(
    dir: &std::path::Path,
    name: &str,
    contents: &[u8],
) -> Result<()> {
    use std::io::Write;

    let final_path = dir.join(name);
    let tmp_path = dir.join(format!("{name}.tmp"));
    let mut file = std::fs::File::create(&tmp_path)?;
    file.write_all(contents)?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(&tmp_path, &final_path)?;
    fsync_dir(dir)?;
    Ok(())
}
