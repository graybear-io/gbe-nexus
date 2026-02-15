# KV State Store Abstraction

**Date**: 2026-02-14
**Context**: Companion to the transport layer. Streams carry signals;
KV carries state. Referenced by task orchestration, sweeper, and archiver.

---

## Why Separate from Transport

The transport moves messages. The KV store answers questions:
- "What state is job X in?"
- "Which jobs have been stuck for > 5 minutes?"
- "What's the last known step for this task?"

These are read-heavy, point-lookup patterns — fundamentally different from
stream consumption. Mixing them into the transport interface would violate
the "transport is a dumb pipe" principle.

---

## Design Goals

1. Simple key-value with TTL — not a general-purpose database
2. Support hash-style fields (job state is multiple fields, not one blob)
3. Scan/filter by key pattern (sweeper needs "all jobs of type X")
4. Same swappability as transport: Redis now, NATS KV or other later

## Non-Goals

- Complex queries (use a database)
- Transactions across multiple keys (design around it)
- Large value storage (claim check to object store)

---

## Core Interface

```
interface StateStore {
    # Lifecycle
    connect(config: StateStoreConfig) -> StateStore
    close()

    # Single key operations
    get(key: string) -> Record?
    put(key: string, record: Record, ttl: duration?)
    delete(key: string)

    # Field-level operations (avoid read-modify-write for concurrent updates)
    get_field(key: string, field: string) -> bytes?
    set_field(key: string, field: string, value: bytes)
    set_fields(key: string, fields: map<string, bytes>)

    # Atomic operations
    compare_and_swap(key: string, field: string, expected: bytes, new: bytes) -> bool

    # Scan (for sweeper)
    scan(prefix: string, filter: ScanFilter?) -> Iterator<(string, Record)>

    # Health
    ping() -> bool
}
```

### Record

```
Record {
    fields  : map<string, bytes>    # arbitrary field/value pairs
    ttl     : duration?             # remaining TTL (if set)
}
```

### ScanFilter

```
ScanFilter {
    field       : string            # filter on this field
    op          : "eq" | "lt" | "gt"
    value       : bytes             # compare against this
    max_results : uint32?           # limit scan size
}
```

**Note on ScanFilter**: Redis supports this via Lua or application-level
filtering over HSCAN. NATS KV supports key-based watches. Keep filters
simple — the sweeper's needs are modest (find jobs where `state != terminal`
and `updated_at < now - timeout`). If filter needs grow complex, that's a
signal to use a database instead.

---

## Key Namespace

Same dot-delimited convention as transport subjects, `:` delimited for Redis.

```
gbe.state.tasks.{task_type}.{task_id}
```

Redis key: `gbe:state:tasks:email-send:job_abc123`

### Task State Record Fields

```
Fields for a task state record:

    state       : string        # "pending" | "claimed" | "running" | "completed" | "failed" | "cancelled"
    task_type   : string        # e.g. "email-send"
    task_id     : string        # unique job identifier
    worker      : string?       # worker that claimed it
    current_step: uint?         # current/last step number
    step_count  : uint?         # total steps (if known)
    created_at  : uint64        # unix millis
    updated_at  : uint64        # unix millis, updated on every state change
    timeout_at  : uint64?       # unix millis, when this job is considered stuck
    error       : string?       # last error message (if failed)
    params_ref  : string?       # claim check reference for job input (if large)
    result_ref  : string?       # claim check reference for job output (if large)
```

`updated_at` is the sweeper's primary query field — "find all records
where `updated_at < now - threshold` and `state` is not terminal."

### Why Fields, Not a Single Blob

Multiple fields allow:
- `set_field("state", "running")` without reading the whole record
- `compare_and_swap("state", "pending", "claimed")` for safe worker claim
- Sweeper reads only `state` + `updated_at` without deserializing params

If this were a single serialized blob, every update would require
read → deserialize → modify → serialize → write, with race conditions
between concurrent updaters.

---

## Key Operations by Consumer

### Worker claiming a job
```
# Atomic: only one worker wins
success = store.compare_and_swap(
    key:      "gbe.state.tasks.email-send.job_123",
    field:    "state",
    expected: "pending",
    new:      "claimed"
)
if success:
    store.set_fields(key, {
        "worker": worker_id,
        "updated_at": now(),
        "timeout_at": now() + step_timeout,
    })
```

### Step completion
```
store.set_fields("gbe.state.tasks.email-send.job_123", {
    "state":        "running",
    "current_step": step + 1,
    "updated_at":   now(),
    "timeout_at":   now() + step_timeout,
})
# Then publish step event to progress stream
transport.publish("gbe.tasks.email-send.progress", step_event)
```

### Job completion
```
store.set_fields("gbe.state.tasks.email-send.job_123", {
    "state":      "completed",
    "updated_at": now(),
    "timeout_at": nil,          # clear timeout
    "result_ref": "s3://...",   # if result is large
})
# Then publish terminal event
transport.publish("gbe.tasks.email-send.terminal", complete_event)
```

### Sweeper scan
```
for (key, record) in store.scan("gbe.state.tasks.", filter: {
    field: "updated_at",
    op: "lt",
    value: now() - stuck_threshold,
}):
    if record.fields["state"] not in ["completed", "failed", "cancelled"]:
        # Job is stuck — republish to queue
        transport.publish("gbe.tasks.{type}.queue", retry_event)
        store.set_field(key, "updated_at", now())  # reset timeout
```

---

## Redis Implementation Notes

| StateStore concept | Redis implementation |
|---|---|
| `get` | `HGETALL {key}` |
| `put` | `HSET {key} {f1} {v1} ...` + `PEXPIRE {key} {ttl}` |
| `delete` | `DEL {key}` |
| `get_field` | `HGET {key} {field}` |
| `set_field` | `HSET {key} {field} {value}` |
| `set_fields` | `HSET {key} {f1} {v1} {f2} {v2} ...` |
| `compare_and_swap` | Lua script: `if HGET == expected then HSET; return 1 else return 0` |
| `scan` | `SCAN` with match pattern + `HGET` per key (or pipeline) |
| `ttl` | `PEXPIRE` / `PERSIST` |

### CAS Lua Script
```lua
-- compare_and_swap.lua
local current = redis.call('HGET', KEYS[1], ARGV[1])
if current == ARGV[2] then
    redis.call('HSET', KEYS[1], ARGV[1], ARGV[3])
    return 1
else
    return 0
end
```

### Scan Performance

`SCAN` + per-key `HGET` is O(N) and doesn't scale elegantly. For the
sweeper this is acceptable — it runs periodically (every 30-60s), not on
the hot path. If task volume grows to where scan is too slow:
- Add a secondary index: a sorted set `gbe:idx:tasks:by_updated` scored
  by `updated_at`, queried with `ZRANGEBYSCORE`.
- Or move sweeper queries to a database.

Start without the index. Add it when scan takes > 1s.

---

## NATS KV Implementation Notes

| StateStore concept | NATS KV implementation |
|---|---|
| `get` | `kv.Get(key)` — returns single value (not fields) |
| `put` | `kv.Put(key, value)` |
| `delete` | `kv.Delete(key)` |
| `compare_and_swap` | `kv.Update(key, value, last_revision)` — revision-based CAS |
| `scan` | `kv.Keys(prefix)` + `kv.Get` per key |
| `ttl` | Bucket-level TTL only (not per-key) |

### Limitations vs Redis
- NATS KV stores opaque values, not hashes — field-level ops require
  serialize/deserialize the whole record.
- TTL is per-bucket, not per-key — would need separate buckets for
  different TTL requirements, or application-level expiry.
- CAS is revision-based (optimistic concurrency) rather than value-based.

**Implication**: The Redis implementation is a better fit for the
field-level access patterns. NATS KV works but with more
application-level logic. This is fine — we're starting on Redis anyway.

---

## What the State Store Does NOT Do

- **Message delivery**: that's the transport
- **Event history**: streams are the event log; KV is current state only
- **Complex queries**: if you need joins or aggregations, use a database
- **Cross-key transactions**: design workflows to use CAS on single keys
