//! Explicit little-endian binary codec + FNV-1a-64 checksums.
//!
//! The on-disk format is written field-by-field in little-endian byte order
//! (see docs/FORMAT.md). Nothing is ever memory-dumped from Rust structs, so
//! there are no padding / ABI portability concerns.

use crate::error::{Result, VectorError};

/// FNV-1a 64-bit checksum. Cheap, portable, and only used to *detect*
/// corruption (not to protect against adversaries).
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET;
    for &byte in bytes {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

/// Append-only little-endian writer over a `Vec<u8>` buffer.
#[derive(Debug, Default)]
pub struct Writer {
    buf: Vec<u8>,
}

impl Writer {
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

    pub fn len(&self) -> usize {
        self.buf.len()
    }

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
    use super::{fnv1a64, Reader, Writer};
    use crate::error::{Result, VectorError};

    pub const TRAILER_LEN: usize = 8;

    /// Write `payload` framed with the given magic and version.
    pub fn encode(magic: &[u8; 4], version: u32, payload: &[u8]) -> Vec<u8> {
        let mut writer = Writer::with_capacity(payload.len() + 20);
        writer.bytes(magic);
        writer.u32(version);
        writer.u64(payload.len() as u64);
        writer.bytes(payload);
        let checksum = fnv1a64(&writer.snapshot());
        writer.u64(checksum);
        writer.into_bytes()
    }

    /// Validate framing, return `(version, payload)`.
    pub fn decode(magic: &[u8; 4], file: &'static str, data: &[u8]) -> Result<(u32, &[u8])> {
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
