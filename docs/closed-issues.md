# Closed Issues

Historical record captured before project restructure (2026-02-15).

## Implement Redis KV state store backend

- **Original ID**: gbe-transport-baa
- **Type**: task
- **Priority**: P1
- **Owner**: Mike Taylor
- **Created**: 2026-02-14
- **Close reason**: Redis KV state store fully implemented
- **Blocked**: gbe-transport-kf7 (archiver), gbe-transport-y8f (sweeper)

### Notes

Implemented RedisStateStore in crates/state-store-redis/. Uses Redis hashes
(HGETALL/HSET/HGET/DEL), Lua script for atomic CAS, SCAN+HGETALL with
client-side filtering for prefix queries. 12 integration tests passing.
ConnectionManager for auto-reconnect. Follows same patterns as transport-redis.

---

## Implement sweeper process

- **Original ID**: gbe-transport-y8f
- **Type**: task
- **Priority**: P2
- **Owner**: Mike Taylor
- **Created**: 2026-02-14
- **Close reason**: Sweeper implemented with stuck job detection, stream trimming, distributed lock
- **Depended on**: gbe-transport-baa (state store)

### Notes

Implemented Sweeper in crates/sweeper/. Stuck job detection via state store
scan with retry budget tracking. Stream trimming via new
Transport::trim_stream() method (XTRIM MINID ~). Distributed lock via Redis
SET NX PX with Lua-guarded release. Dead consumer cleanup deferred. 7
integration tests passing. Also extended Transport trait and RedisTransport
with trim_stream.

---

## Implement archiver process

- **Original ID**: gbe-transport-kf7
- **Type**: task
- **Priority**: P2
- **Owner**: Mike Taylor
- **Created**: 2026-02-14
- **Close reason**: Archiver implemented with batch-then-ack, gzipped JSONL, ArchiveWriter trait
- **Depended on**: gbe-transport-baa (state store)

### Notes

Implemented Archiver in crates/sweeper/. Direct XREADGROUP for batch-then-ack
(no MessageHandler). ArchiveWriter trait with FsArchiveWriter for testing (S3
impl deferred). Gzipped JSONL output with daily-partitioned paths
(domain/YYYY/MM/DD/batch_id.jsonl.gz). Raw envelope JSON pass-through (no
deserialization round-trip). 5 integration tests. ArchiverConfig +
ArchivalStream for per-stream batch_size/batch_timeout.

---

## Domain schemas (per consuming project)

- **Original ID**: gbe-transport-7u9
- **Type**: task
- **Priority**: P3
- **Owner**: Mike Taylor
- **Created**: 2026-02-14
- **Close reason**: DomainPayload\<T\> added to transport crate — enforces v/ts/id contract

### Notes

Each consuming project brings its own payload schema. Required fields: v
(schema version), ts (event time), id (dedup key). Shared type defs in
Rust/Go/Python. No registry. Implemented as generic DomainPayload\<T\> wrapper
struct in the transport crate with to_bytes/from_bytes helpers.
