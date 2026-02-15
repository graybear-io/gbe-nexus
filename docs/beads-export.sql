PRAGMA foreign_keys=OFF;
BEGIN TRANSACTION;
CREATE TABLE issues (
    id TEXT PRIMARY KEY,
    content_hash TEXT,
    title TEXT NOT NULL CHECK(length(title) <= 500),
    description TEXT NOT NULL DEFAULT '',
    design TEXT NOT NULL DEFAULT '',
    acceptance_criteria TEXT NOT NULL DEFAULT '',
    notes TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'open',
    priority INTEGER NOT NULL DEFAULT 2 CHECK(priority >= 0 AND priority <= 4),
    issue_type TEXT NOT NULL DEFAULT 'task',
    assignee TEXT,
    estimated_minutes INTEGER,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_by TEXT DEFAULT '',
    owner TEXT DEFAULT '',
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    closed_at DATETIME,
    closed_by_session TEXT DEFAULT '',
    external_ref TEXT,
    compaction_level INTEGER DEFAULT 0,
    compacted_at DATETIME,
    compacted_at_commit TEXT,
    original_size INTEGER,
    deleted_at DATETIME,
    deleted_by TEXT DEFAULT '',
    delete_reason TEXT DEFAULT '',
    original_type TEXT DEFAULT '',
    -- Messaging fields (bd-kwro)
    sender TEXT DEFAULT '',
    ephemeral INTEGER DEFAULT 0,
    -- Pinned field (bd-7h5)
    pinned INTEGER DEFAULT 0,
    -- Template field (beads-1ra)
    is_template INTEGER DEFAULT 0,
    -- Work economics field (bd-fqze8) - HOP Decision 006
    crystallizes INTEGER DEFAULT 0,
    -- Molecule type field (bd-oxgi)
    mol_type TEXT DEFAULT '',
    -- Work type field (Decision 006: mutex vs open_competition)
    work_type TEXT DEFAULT 'mutex',
    -- HOP quality score field (0.0-1.0, set by Refineries on merge)
    quality_score REAL,
    -- Federation source system field
    source_system TEXT DEFAULT '',
    -- Event fields (bd-ecmd)
    event_kind TEXT DEFAULT '',
    actor TEXT DEFAULT '',
    target TEXT DEFAULT '',
    payload TEXT DEFAULT '', source_repo TEXT DEFAULT '.', close_reason TEXT DEFAULT '', await_type TEXT, await_id TEXT, timeout_ns INTEGER, waiters TEXT, hook_bead TEXT DEFAULT '', role_bead TEXT DEFAULT '', agent_state TEXT DEFAULT '', last_activity DATETIME, role_type TEXT DEFAULT '', rig TEXT DEFAULT '', due_at DATETIME, defer_until DATETIME,
    -- NOTE: replies_to, relates_to, duplicate_of, superseded_by removed per Decision 004
    -- These relationships are now stored in the dependencies table
    -- closed_at constraint: closed issues must have it, tombstones may retain it from before deletion
    CHECK (
        (status = 'closed' AND closed_at IS NOT NULL) OR
        (status = 'tombstone') OR
        (status NOT IN ('closed', 'tombstone') AND closed_at IS NULL)
    )
);
INSERT INTO issues VALUES('gbe-transport-baa','4e0a5cb5d9a2c98884b5eb55df312ce161fb5f5bdafe09e9566b4a79ccc915d0','Implement Redis KV state store backend','','','','Implemented RedisStateStore in crates/state-store-redis/. Uses Redis hashes (HGETALL/HSET/HGET/DEL), Lua script for atomic CAS, SCAN+HGETALL with client-side filtering for prefix queries. 12 integration tests passing. ConnectionManager for auto-reconnect. Follows same patterns as transport-redis.','closed',1,'task','',NULL,'2026-02-14T22:11:13.938086-05:00','Mike Taylor','bear@bear.im','2026-02-14T22:50:04.22293-05:00','2026-02-14T22:49:59.24458-05:00','',NULL,0,NULL,NULL,NULL,NULL,'','','','',0,0,0,0,'','mutex',NULL,'','','','','','.','Redis KV state store fully implemented','','',0,'','','','',NULL,'','',NULL,NULL);
INSERT INTO issues VALUES('gbe-transport-y8f','e767ceee70ab345e78227f9a2bafb6f974f7785567f5a92f26ae572ef2fb3600','Implement sweeper process','','','','Implemented Sweeper in crates/sweeper/. Stuck job detection via state store scan with retry budget tracking. Stream trimming via new Transport::trim_stream() method (XTRIM MINID ~). Distributed lock via Redis SET NX PX with Lua-guarded release. Dead consumer cleanup deferred. 7 integration tests passing. Also extended Transport trait and RedisTransport with trim_stream.','closed',2,'task','',NULL,'2026-02-14T22:11:14.727075-05:00','Mike Taylor','bear@bear.im','2026-02-15T12:19:20.527547-05:00','2026-02-15T12:19:15.342997-05:00','',NULL,0,NULL,NULL,NULL,NULL,'','','','',0,0,0,0,'','mutex',NULL,'','','','','','.','Sweeper implemented with stuck job detection, stream trimming, distributed lock','','',0,'','','','',NULL,'','',NULL,NULL);
INSERT INTO issues VALUES('gbe-transport-kf7','073b92ca72cb83866410d9220f8acf6af476762fa77c633c901838ed45150157','Implement archiver process','','','','Implemented Archiver in crates/sweeper/. Direct XREADGROUP for batch-then-ack (no MessageHandler). ArchiveWriter trait with FsArchiveWriter for testing (S3 impl deferred). Gzipped JSONL output with daily-partitioned paths (domain/YYYY/MM/DD/batch_id.jsonl.gz). Raw envelope JSON pass-through (no deserialization round-trip). 5 integration tests. ArchiverConfig + ArchivalStream for per-stream batch_size/batch_timeout.','closed',2,'task','',NULL,'2026-02-14T22:11:15.31264-05:00','Mike Taylor','bear@bear.im','2026-02-15T12:48:21.715992-05:00','2026-02-15T12:48:14.855698-05:00','',NULL,0,NULL,NULL,NULL,NULL,'','','','',0,0,0,0,'','mutex',NULL,'','','','','','.','Archiver implemented with batch-then-ack, gzipped JSONL, ArchiveWriter trait','','',0,'','','','',NULL,'','',NULL,NULL);
INSERT INTO issues VALUES('gbe-transport-7u9','81e7ac49e514ac22209e3a5668ee48599059648974fcb68001e044863f54fa30','Domain schemas (per consuming project)','','','','Each consuming project brings its own payload schema. Required fields: v (schema version), ts (event time), id (dedup key). Shared type defs in Rust/Go/Python. No registry. Deferred until first consumer onboards.','closed',3,'task','',NULL,'2026-02-14T22:11:15.89317-05:00','Mike Taylor','bear@bear.im','2026-02-15T13:27:18.455891-05:00','2026-02-15T13:27:18.455891-05:00','',NULL,0,NULL,NULL,NULL,NULL,'','','','',0,0,0,0,'','mutex',NULL,'','','','','','.','DomainPayload<T> added to transport crate — enforces v/ts/id contract','','',0,'','','','',NULL,'','',NULL,NULL);
INSERT INTO issues VALUES('gbe-transport-07d','910d89cb30ba98bd504e065c4e93175728f2e490472f9b3eeff47cd42f2657ad','Bridge adapter between gbe and gbe-transport','','','','Thin GBE adapter that subscribes to gbe-transport streams and bridges into the local gbe tool composition layer (Unix sockets). Different layers, no code overlap. Deferred until both projects have stable interfaces.','open',3,'task','',NULL,'2026-02-14T22:11:16.576756-05:00','Mike Taylor','bear@bear.im','2026-02-14T22:11:33.617415-05:00',NULL,'',NULL,0,NULL,NULL,NULL,NULL,'','','','',0,0,0,0,'','mutex',NULL,'','','','','','.','','','',0,'','','','',NULL,'','',NULL,NULL);
CREATE TABLE comments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    issue_id TEXT NOT NULL,
    author TEXT NOT NULL,
    text TEXT NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (issue_id) REFERENCES issues(id) ON DELETE CASCADE
);
CREATE TABLE events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    issue_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    actor TEXT NOT NULL,
    old_value TEXT,
    new_value TEXT,
    comment TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (issue_id) REFERENCES issues(id) ON DELETE CASCADE
);
INSERT INTO events VALUES(1,'gbe-transport-baa','created','Mike Taylor',NULL,'{"id":"gbe-transport-baa","title":"Implement Redis KV state store backend","status":"open","priority":1,"issue_type":"task","owner":"bear@bear.im","created_at":"2026-02-14T22:11:13.938086-05:00","created_by":"Mike Taylor","updated_at":"2026-02-14T22:11:13.938086-05:00"}',NULL,'2026-02-15 03:11:13');
INSERT INTO events VALUES(2,'gbe-transport-y8f','created','Mike Taylor',NULL,'{"id":"gbe-transport-y8f","title":"Implement sweeper process","status":"open","priority":2,"issue_type":"task","owner":"bear@bear.im","created_at":"2026-02-14T22:11:14.727075-05:00","created_by":"Mike Taylor","updated_at":"2026-02-14T22:11:14.727075-05:00"}',NULL,'2026-02-15 03:11:14');
INSERT INTO events VALUES(3,'gbe-transport-kf7','created','Mike Taylor',NULL,'{"id":"gbe-transport-kf7","title":"Implement archiver process","status":"open","priority":2,"issue_type":"task","owner":"bear@bear.im","created_at":"2026-02-14T22:11:15.31264-05:00","created_by":"Mike Taylor","updated_at":"2026-02-14T22:11:15.31264-05:00"}',NULL,'2026-02-15 03:11:15');
INSERT INTO events VALUES(4,'gbe-transport-7u9','created','Mike Taylor',NULL,'{"id":"gbe-transport-7u9","title":"Domain schemas (per consuming project)","status":"open","priority":3,"issue_type":"task","owner":"bear@bear.im","created_at":"2026-02-14T22:11:15.89317-05:00","created_by":"Mike Taylor","updated_at":"2026-02-14T22:11:15.89317-05:00"}',NULL,'2026-02-15 03:11:15');
INSERT INTO events VALUES(5,'gbe-transport-07d','created','Mike Taylor',NULL,'{"id":"gbe-transport-07d","title":"Bridge adapter between gbe and gbe-transport","status":"open","priority":3,"issue_type":"task","owner":"bear@bear.im","created_at":"2026-02-14T22:11:16.576756-05:00","created_by":"Mike Taylor","updated_at":"2026-02-14T22:11:16.576756-05:00"}',NULL,'2026-02-15 03:11:16');
INSERT INTO events VALUES(6,'gbe-transport-baa','updated','Mike Taylor','{"id":"gbe-transport-baa","title":"Implement Redis KV state store backend","status":"open","priority":1,"issue_type":"task","owner":"bear@bear.im","created_at":"2026-02-14T22:11:13.938086-05:00","created_by":"Mike Taylor","updated_at":"2026-02-14T22:11:13.938086-05:00"}','{"notes":"Implement StateStore trait for Redis. Uses HSET/HGETALL for records, HGET/HSET for field-level ops, Lua script for compare_and_swap, SCAN+HGET for sweeper queries. Design doc: docs/research/kv-state-store.md. Crate: crates/state-store-redis/"}',NULL,'2026-02-15 03:11:24');
INSERT INTO events VALUES(7,'gbe-transport-y8f','updated','Mike Taylor','{"id":"gbe-transport-y8f","title":"Implement sweeper process","status":"open","priority":2,"issue_type":"task","owner":"bear@bear.im","created_at":"2026-02-14T22:11:14.727075-05:00","created_by":"Mike Taylor","updated_at":"2026-02-14T22:11:14.727075-05:00"}','{"notes":"Stuck job detection via KV state scan, Redis stream trimming via XTRIM, dead consumer cleanup. Runs every 30-60s with distributed lock. Depends on transport-redis and state-store-redis. Design doc: docs/research/sweeper-archiver.md. Crate: crates/sweeper/"}',NULL,'2026-02-15 03:11:26');
INSERT INTO events VALUES(8,'gbe-transport-kf7','updated','Mike Taylor','{"id":"gbe-transport-kf7","title":"Implement archiver process","status":"open","priority":2,"issue_type":"task","owner":"bear@bear.im","created_at":"2026-02-14T22:11:15.31264-05:00","created_by":"Mike Taylor","updated_at":"2026-02-14T22:11:15.31264-05:00"}','{"notes":"Drain archival streams (audit) to S3 as gzipped JSONL. Batched writes, ack after successful S3 PUT. Daily partitioned keys. Depends on transport-redis. Design doc: docs/research/sweeper-archiver.md. Crate: crates/sweeper/"}',NULL,'2026-02-15 03:11:28');
INSERT INTO events VALUES(9,'gbe-transport-7u9','updated','Mike Taylor','{"id":"gbe-transport-7u9","title":"Domain schemas (per consuming project)","status":"open","priority":3,"issue_type":"task","owner":"bear@bear.im","created_at":"2026-02-14T22:11:15.89317-05:00","created_by":"Mike Taylor","updated_at":"2026-02-14T22:11:15.89317-05:00"}','{"notes":"Each consuming project brings its own payload schema. Required fields: v (schema version), ts (event time), id (dedup key). Shared type defs in Rust/Go/Python. No registry. Deferred until first consumer onboards."}',NULL,'2026-02-15 03:11:31');
INSERT INTO events VALUES(10,'gbe-transport-07d','updated','Mike Taylor','{"id":"gbe-transport-07d","title":"Bridge adapter between gbe and gbe-transport","status":"open","priority":3,"issue_type":"task","owner":"bear@bear.im","created_at":"2026-02-14T22:11:16.576756-05:00","created_by":"Mike Taylor","updated_at":"2026-02-14T22:11:16.576756-05:00"}','{"notes":"Thin GBE adapter that subscribes to gbe-transport streams and bridges into the local gbe tool composition layer (Unix sockets). Different layers, no code overlap. Deferred until both projects have stable interfaces."}',NULL,'2026-02-15 03:11:33');
INSERT INTO events VALUES(11,'gbe-transport-y8f','dependency_added','Mike Taylor',NULL,NULL,'Added dependency: gbe-transport-y8f blocks gbe-transport-baa','2026-02-15 03:11:46');
INSERT INTO events VALUES(12,'gbe-transport-kf7','dependency_added','Mike Taylor',NULL,NULL,'Added dependency: gbe-transport-kf7 blocks gbe-transport-baa','2026-02-15 03:11:46');
INSERT INTO events VALUES(13,'gbe-transport-baa','status_changed','Mike Taylor','{"id":"gbe-transport-baa","title":"Implement Redis KV state store backend","notes":"Implement StateStore trait for Redis. Uses HSET/HGETALL for records, HGET/HSET for field-level ops, Lua script for compare_and_swap, SCAN+HGET for sweeper queries. Design doc: docs/research/kv-state-store.md. Crate: crates/state-store-redis/","status":"open","priority":1,"issue_type":"task","owner":"bear@bear.im","created_at":"2026-02-14T22:11:13.938086-05:00","created_by":"Mike Taylor","updated_at":"2026-02-14T22:11:24.263032-05:00"}','{"status":"in_progress"}',NULL,'2026-02-15 03:42:53');
INSERT INTO events VALUES(14,'gbe-transport-baa','closed','Mike Taylor',NULL,NULL,'Redis KV state store fully implemented','2026-02-15 03:49:59');
INSERT INTO events VALUES(15,'gbe-transport-baa','updated','Mike Taylor','{"id":"gbe-transport-baa","title":"Implement Redis KV state store backend","notes":"Implement StateStore trait for Redis. Uses HSET/HGETALL for records, HGET/HSET for field-level ops, Lua script for compare_and_swap, SCAN+HGET for sweeper queries. Design doc: docs/research/kv-state-store.md. Crate: crates/state-store-redis/","status":"closed","priority":1,"issue_type":"task","owner":"bear@bear.im","created_at":"2026-02-14T22:11:13.938086-05:00","created_by":"Mike Taylor","updated_at":"2026-02-14T22:49:59.24458-05:00","closed_at":"2026-02-14T22:49:59.24458-05:00","close_reason":"Redis KV state store fully implemented"}','{"notes":"Implemented RedisStateStore in crates/state-store-redis/. Uses Redis hashes (HGETALL/HSET/HGET/DEL), Lua script for atomic CAS, SCAN+HGETALL with client-side filtering for prefix queries. 12 integration tests passing. ConnectionManager for auto-reconnect. Follows same patterns as transport-redis."}',NULL,'2026-02-15 03:50:04');
INSERT INTO events VALUES(16,'gbe-transport-y8f','status_changed','Mike Taylor','{"id":"gbe-transport-y8f","title":"Implement sweeper process","notes":"Stuck job detection via KV state scan, Redis stream trimming via XTRIM, dead consumer cleanup. Runs every 30-60s with distributed lock. Depends on transport-redis and state-store-redis. Design doc: docs/research/sweeper-archiver.md. Crate: crates/sweeper/","status":"open","priority":2,"issue_type":"task","owner":"bear@bear.im","created_at":"2026-02-14T22:11:14.727075-05:00","created_by":"Mike Taylor","updated_at":"2026-02-14T22:11:26.56241-05:00"}','{"status":"in_progress"}',NULL,'2026-02-15 03:56:11');
INSERT INTO events VALUES(17,'gbe-transport-y8f','closed','Mike Taylor',NULL,NULL,'Sweeper implemented with stuck job detection, stream trimming, distributed lock','2026-02-15 17:19:15');
INSERT INTO events VALUES(18,'gbe-transport-y8f','updated','Mike Taylor','{"id":"gbe-transport-y8f","title":"Implement sweeper process","notes":"Stuck job detection via KV state scan, Redis stream trimming via XTRIM, dead consumer cleanup. Runs every 30-60s with distributed lock. Depends on transport-redis and state-store-redis. Design doc: docs/research/sweeper-archiver.md. Crate: crates/sweeper/","status":"closed","priority":2,"issue_type":"task","owner":"bear@bear.im","created_at":"2026-02-14T22:11:14.727075-05:00","created_by":"Mike Taylor","updated_at":"2026-02-15T12:19:15.342997-05:00","closed_at":"2026-02-15T12:19:15.342997-05:00","close_reason":"Sweeper implemented with stuck job detection, stream trimming, distributed lock"}','{"notes":"Implemented Sweeper in crates/sweeper/. Stuck job detection via state store scan with retry budget tracking. Stream trimming via new Transport::trim_stream() method (XTRIM MINID ~). Distributed lock via Redis SET NX PX with Lua-guarded release. Dead consumer cleanup deferred. 7 integration tests passing. Also extended Transport trait and RedisTransport with trim_stream."}',NULL,'2026-02-15 17:19:20');
INSERT INTO events VALUES(19,'gbe-transport-kf7','status_changed','Mike Taylor','{"id":"gbe-transport-kf7","title":"Implement archiver process","notes":"Drain archival streams (audit) to S3 as gzipped JSONL. Batched writes, ack after successful S3 PUT. Daily partitioned keys. Depends on transport-redis. Design doc: docs/research/sweeper-archiver.md. Crate: crates/sweeper/","status":"open","priority":2,"issue_type":"task","owner":"bear@bear.im","created_at":"2026-02-14T22:11:15.31264-05:00","created_by":"Mike Taylor","updated_at":"2026-02-14T22:11:28.812688-05:00"}','{"status":"in_progress"}',NULL,'2026-02-15 17:27:07');
INSERT INTO events VALUES(20,'gbe-transport-kf7','closed','Mike Taylor',NULL,NULL,'Archiver implemented with batch-then-ack, gzipped JSONL, ArchiveWriter trait','2026-02-15 17:48:14');
INSERT INTO events VALUES(21,'gbe-transport-kf7','updated','Mike Taylor','{"id":"gbe-transport-kf7","title":"Implement archiver process","notes":"Drain archival streams (audit) to S3 as gzipped JSONL. Batched writes, ack after successful S3 PUT. Daily partitioned keys. Depends on transport-redis. Design doc: docs/research/sweeper-archiver.md. Crate: crates/sweeper/","status":"closed","priority":2,"issue_type":"task","owner":"bear@bear.im","created_at":"2026-02-14T22:11:15.31264-05:00","created_by":"Mike Taylor","updated_at":"2026-02-15T12:48:14.855698-05:00","closed_at":"2026-02-15T12:48:14.855698-05:00","close_reason":"Archiver implemented with batch-then-ack, gzipped JSONL, ArchiveWriter trait"}','{"notes":"Implemented Archiver in crates/sweeper/. Direct XREADGROUP for batch-then-ack (no MessageHandler). ArchiveWriter trait with FsArchiveWriter for testing (S3 impl deferred). Gzipped JSONL output with daily-partitioned paths (domain/YYYY/MM/DD/batch_id.jsonl.gz). Raw envelope JSON pass-through (no deserialization round-trip). 5 integration tests. ArchiverConfig + ArchivalStream for per-stream batch_size/batch_timeout."}',NULL,'2026-02-15 17:48:21');
INSERT INTO events VALUES(22,'gbe-transport-7u9','status_changed','Mike Taylor','{"id":"gbe-transport-7u9","title":"Domain schemas (per consuming project)","notes":"Each consuming project brings its own payload schema. Required fields: v (schema version), ts (event time), id (dedup key). Shared type defs in Rust/Go/Python. No registry. Deferred until first consumer onboards.","status":"open","priority":3,"issue_type":"task","owner":"bear@bear.im","created_at":"2026-02-14T22:11:15.89317-05:00","created_by":"Mike Taylor","updated_at":"2026-02-14T22:11:31.213777-05:00"}','{"status":"in_progress"}',NULL,'2026-02-15 18:19:47');
INSERT INTO events VALUES(23,'gbe-transport-7u9','closed','Mike Taylor',NULL,NULL,'DomainPayload<T> added to transport crate — enforces v/ts/id contract','2026-02-15 18:27:18');
CREATE TABLE IF NOT EXISTS "dependencies" (
			issue_id TEXT NOT NULL,
			depends_on_id TEXT NOT NULL,
			type TEXT NOT NULL DEFAULT 'blocks',
			created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
			created_by TEXT NOT NULL,
			metadata TEXT,
			thread_id TEXT,
			PRIMARY KEY (issue_id, depends_on_id, type),
			FOREIGN KEY (issue_id) REFERENCES issues(id) ON DELETE CASCADE
		);
INSERT INTO dependencies VALUES('gbe-transport-y8f','gbe-transport-baa','blocks','2026-02-14T22:11:46.310766-05:00','Mike Taylor','','');
INSERT INTO dependencies VALUES('gbe-transport-kf7','gbe-transport-baa','blocks','2026-02-14T22:11:46.500567-05:00','Mike Taylor','','');
COMMIT;
