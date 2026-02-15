# Sweeper & Archiver Design

**Date**: 2026-02-14
**Context**: Two related but distinct background processes that maintain
the health of the transport and state layers.

---

## Two Processes, Two Jobs

```
Sweeper:  "is anything stuck or expired?"   → acts on live state
Archiver: "does anything need cold storage?" → acts on stream data
```

They share infrastructure (transport, state store) but have different
concerns. They can run as separate processes, goroutines, or tasks in
the same service — deployment is an implementation detail.

---

## Sweeper

### Purpose
Detect stuck/expired jobs and enforce stream retention on Redis
(NATS handles retention natively).

### Responsibilities

1. **Stuck job detection**: scan KV state for jobs past their timeout
2. **Stream trimming**: enforce `max_age` on Redis Streams via `XTRIM`
3. **Dead consumer cleanup**: detect consumer groups with no active
   consumers and stale pending messages

### Run Cadence
- Every 30-60 seconds (configurable)
- Single instance per deployment (use a distributed lock if running
  multiple replicas)

### Stuck Job Flow

```
sweeper                         state store              transport
  |                                |                        |
  | scan("gbe.state.tasks.",      |                        |
  |   updated_at < now - timeout, |                        |
  |   state not in terminal)      |                        |
  |------------------------------->|                        |
  |  <-- stuck jobs []------------|                        |
  |                                                         |
  | for each stuck job:                                     |
  |   set_fields(key, {                                     |
  |     state: "pending",                                   |
  |     updated_at: now(),                                  |
  |     timeout_at: now() + timeout,                        |
  |   })                                                    |
  |------------------------------->|                        |
  |                                                         |
  |   publish("gbe.tasks.{type}.queue", retry_event)       |
  |-------------------------------------------------------->|
  |                                                         |
  |   publish("gbe.events.system.sweep", sweep_report)     |
  |-------------------------------------------------------->|
```

### Retry Budget

The sweeper shouldn't retry forever. Each job state record tracks retries:

```
retry_count : uint          # incremented by sweeper each time
max_retries : uint          # from task type config (default 3)
```

When `retry_count >= max_retries`:
- Set state to `"failed"`
- Publish to `*.terminal` stream
- Publish to `gbe.events.system.error` for alerting
- Stop retrying

### Stream Trimming (Redis only)

For each configured stream, trim based on `max_age`:

```
for stream in configured_streams:
    min_id = timestamp_to_redis_id(now() - stream.max_age)
    XTRIM {stream} MINID ~ {min_id}
```

The `~` makes it approximate (faster — Redis doesn't have to be exact
about the boundary). Close enough for retention.

**Not needed for NATS** — JetStream enforces `MaxAge` natively per stream.

### Distributed Lock

If multiple replicas are running, only one sweeper should be active.

```
# Redis implementation
acquired = SET gbe:lock:sweeper {instance_id} NX PX 60000
if acquired:
    run_sweep()
    DEL gbe:lock:sweeper  # or let it expire
```

Simple, good enough. Not bulletproof against split-brain but the worst
case is two sweeps running simultaneously — the CAS on job state
prevents double-claiming.

---

## Archiver

### Purpose
Drain data from streams to cold storage before the sweeper trims it.
Only operates on domains with archival requirements (currently: audit).

### Responsibilities

1. **Consume from archival streams** as a consumer group member
2. **Batch and write to cold storage** (S3, Postgres, etc.)
3. **Confirm successful write before acking** stream messages
4. **Emit archival events** for observability

### Which Streams

Driven by config — each stream declares its archival policy:

```yaml
archival:
  - stream: "gbe.events.audit.*"
    destination: "s3"
    batch_size: 1000            # messages per archive file
    batch_timeout: 60s          # flush partial batch after this
    format: "jsonl"             # one JSON object per line
    partition: "daily"          # one file per day

  # future: add more streams as needed
  # - stream: "gbe.events.auth.*"
  #   destination: "s3"
  #   ...
```

### Archive Flow

```
archiver                        transport              cold storage (S3)
  |                                |                        |
  | subscribe("gbe.events.audit.*",                        |
  |   group: "archiver")          |                        |
  |------------------------------->|                        |
  |                                |                        |
  |  <-- msg batch (up to 1000) --|                        |
  |                                                         |
  | serialize batch to JSONL                                |
  | PUT s3://archive/audit/2026/02/14/batch_001.jsonl.gz   |
  |-------------------------------------------------------->|
  |                                 <-- 200 OK ------------|
  |                                                         |
  | ack all messages in batch      |                        |
  |------------------------------->|                        |
  |                                                         |
  | publish("gbe.events.system.archive", {                 |
  |   stream: "audit",                                      |
  |   count: 1000,                                          |
  |   destination: "s3://...",                               |
  | })                                                      |
  |------------------------------->|                        |
```

### S3 Key Structure

```
s3://{bucket}/archive/{domain}/{YYYY}/{MM}/{DD}/{batch_id}.jsonl.gz
```

Example:
```
s3://gbe-archive/archive/audit/2026/02/14/batch_001.jsonl.gz
s3://gbe-archive/archive/audit/2026/02/14/batch_002.jsonl.gz
```

Daily partitioning enables:
- Athena/Presto queries with partition pruning
- Lifecycle policies per prefix (e.g., move to Glacier after 1 year)
- Simple manual inspection ("what happened on Feb 14?")

### Failure Handling

**S3 write fails**: don't ack the batch. Messages remain pending in the
consumer group. Archiver retries on next cycle. The stream's live
retention window (7d for audit) provides buffer for extended outages.

**Partial batch on shutdown**: flush current batch to S3 before exit.
If the process crashes, unacked messages are redelivered — worst case
is duplicate records in S3. Downstream queries should be idempotent
(deduplicate on `message_id`).

**Archiver falls behind**: monitor lag via `XPENDING` (Redis) or
consumer ack floor (NATS). Alert if lag exceeds a threshold (e.g.,
> 10k pending messages or > 1h behind).

### Ordering and Deduplication

Archive files contain messages in stream order within each batch.
Cross-batch ordering is not guaranteed (batches may overlap during
retries). Downstream consumers should:
- Sort by `timestamp` + `message_id` if order matters
- Deduplicate on `message_id` if retries caused duplicates

---

## How They Relate

```
                     ┌─────────────┐
                     │   Streams   │
                     │ (transport) │
                     └──────┬──────┘
                            │
              ┌─────────────┼─────────────┐
              │             │             │
              ▼             ▼             ▼
        ┌──────────┐  ┌──────────┐  ┌──────────┐
        │ Consumers│  │ Archiver │  │ Sweeper  │
        │ (domain  │  │          │  │          │
        │  workers)│  │ reads    │  │ reads    │
        └──────────┘  │ streams  │  │ KV state │
                      │ writes   │  │ trims    │
                      │ to S3    │  │ streams  │
                      └──────────┘  │ retries  │
                                    │ stuck    │
                      ┌──────────┐  │ jobs     │
                      │ KV State │  └──────────┘
                      │  Store   │       │
                      └──────────┘       │
                           ▲             │
                           └─────────────┘
```

### Ordering of Operations

The sweeper trims streams. The archiver reads streams. If the sweeper
trims before the archiver reads, data is lost.

**Solution**: The archiver runs as a consumer group. It acks messages
after writing to S3. The sweeper's `XTRIM` by time only removes messages
older than `max_age`. As long as the archiver's lag stays within the
retention window, no data is lost.

**Guard rail**: The sweeper checks archiver consumer group lag before
trimming archival streams. If lag exceeds a threshold, skip trimming
that stream and emit an alert.

```
# Before trimming an archival stream
pending = XPENDING gbe:events:audit:change archiver
if pending.count > lag_threshold:
    publish alert to gbe.events.system.error
    skip trim for this stream
else:
    XTRIM as normal
```

---

## Configuration

Both processes share a config file with the stream/retention config:

```yaml
sweeper:
  interval: 30s
  lock_ttl: 60s
  stuck_threshold: 300s         # 5 min default, overridden per task type

archiver:
  streams:
    - stream: "gbe.events.audit.*"
      destination: "s3"
      bucket: "gbe-archive"
      batch_size: 1000
      batch_timeout: 60s
      format: "jsonl"
      compression: "gzip"
      partition: "daily"

retention:
  # sweeper enforces these via XTRIM (Redis)
  # NATS: configured natively per stream via ensure_stream
  "gbe.notify.topic.*":        72h
  "gbe.notify.broadcast.*":    7d
  "gbe.events.system.*":       7d
  "gbe.events.api.*":          3d
  "gbe.events.audit.*":        7d      # live window; archiver handles long-term
  "gbe.tasks.*.queue":         24h
  "gbe.tasks.*.progress":      24h
  "gbe.tasks.*.terminal":      24h
  "gbe._deadletter.*":         7d
```

---

## What These Processes Do NOT Do

- **Business logic**: they don't interpret payloads or make domain decisions
- **Retry policy**: the sweeper re-enqueues; the task layer decides what to do
- **Schema validation**: they pass bytes through, same as transport
- **Query serving**: they're background writers, not query engines. Athena
  or a database handles archive queries.
