# On-disk format v1

All integer fields are little-endian. The implementation never writes Rust struct memory directly. Every framed file has:

```text
[magic: 4 bytes][version: u32][payload_len: u64][payload][checksum: u64]
```

`checksum` is FNV-1a-64 over magic + version + payload length + payload. It detects truncation and accidental bit flips; it is not an authentication code.

## Directory

```text
manifest.bin
vectors.<generation>.dat
ids.<generation>.dat
graph.<generation>.dat
meta.<generation>.dat
wal.log
```

The manifest names the current generation implicitly through its `generation` field and records the frame checksum of every segment. Older generation files are unreferenced garbage and may be removed after the manifest commit.

## Manifest payload

```text
format_version: u32 = 1
generation: u64
dimensions: u32
metric: u8                 0 cosine, 1 dot, 2 l2
algorithm: u8              0 exact, 1 hnsw, 2 adaptive
m: u32
ef_construction: u32
ef_search: u32
heuristic: u8
seed: u64
exact_threshold: u64
row_count: u32
live_count: u64
deleted_count: u64
max_seq: u64
graph_entry_present: u8
graph_entry: u32           only when present
graph_max_level: u32
vectors_checksum: u64
graph_checksum: u64
ids_checksum: u64
meta_checksum: u64
created_at_unix: i64
embedding_space.provider: short string
embedding_space.model: short string
embedding_space.source_hash: short string
```

A frame or inner format version newer than `1` returns `UnsupportedVersion`; it is never guessed. Bad magic, invalid lengths, invalid UTF-8, and checksum failures return `CorruptIndex`.

## vectors

The payload is a row-major flat matrix of `f32` values, each written with `f32::to_le_bytes`. Row `i` begins at `i * dimensions * 4`. The default open path maps the payload read-only through `memmap2`; rows added after the checkpoint are kept in a heap tail until the next checkpoint. Heap mode copies and decodes the payload.

## ids

The payload is `row_count` consecutive 16-byte little-endian `u128` external IDs. IDs are stable across compaction; dense row IDs are internal only.

## metadata

Rows are encoded sequentially:

```text
flags: u8 (bit 0 route, bit 1 source, bit 2 knowledge point, bit 3 kind)
route_id: u128, if present
source_id: u128, if present
knowledge_point_id: u128, if present
kind: u16 byte length + UTF-8 bytes, if present
```

After all rows:

```text
alive_mask_len: u32
alive_mask: ceil(row_count / 8) bytes
```

A cleared alive bit is a tombstone. The idmap is rebuilt only from live rows on open.

## graph

The payload begins with:

```text
m: u32
entry_present: u8
entry: u32, if present
max_level: u32
node_count: u32
levels[node_count]: u32
links: fixed-capacity u32 slots
```

Each node at level `L` owns `2M + L*M` slots: `2M` at layer 0 and `M` per upper layer. Empty slots are `u32::MAX`. Link targets must be less than `node_count` or equal to the sentinel. Offsets are reconstructed from levels; they are not trusted from disk.

## WAL

`wal.log` is a concatenation of the same framed format with magic `VWL1`. A record payload is:

```text
op: u8                    1 insert/upsert, 2 delete
seq: u64
insert: VectorId + u32 dimension + f32 values + metadata
 delete: VectorId
```

Records must have strictly increasing sequence numbers. On open, records at or below the manifest's `max_seq` are already represented in the checkpoint and are skipped. Later records are replayed in order. An incomplete trailing frame is treated as a crash-torn write and truncated to the valid prefix. A complete frame with a bad checksum or invalid payload is corruption.

## Atomicity and safety

Segment files are written to `.tmp`, fsynced, and renamed individually before the manifest commit. The manifest rename is the commit point. A checkpoint crash cannot make the previous manifest point at partially written files. The sole mmap call is isolated in `persist.rs` with an inline SAFETY argument; all engine logic is safe Rust.
