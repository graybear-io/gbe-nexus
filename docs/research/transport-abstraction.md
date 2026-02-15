# Transport Abstraction Layer

**Date**: 2026-02-14
**Context**: Abstraction over Redis Streams (MemoryDB) and NATS JetStream.
Must support Rust, Go, and Python implementations.

---

## Design Goals

1. Application code never touches transport-specific APIs
2. Swap Redis ↔ NATS without changing domain logic
3. Envelope/payload separation — transport handles envelope, domains own payload
4. Consumer groups with explicit ack/nak
5. Dead-letter routing for unprocessable messages

## Non-Goals

- Abstracting KV store (separate interface, simpler problem)
- Transaction semantics across multiple publishes
- Request/reply (use publish + subscribe patterns instead)

---

## Envelope

Every message on the wire has the same outer structure. The transport
creates/reads the envelope. Domain code only sees the payload.

```
Envelope {
    message_id  : string        # unique, transport-generated (ULID or equivalent)
    subject     : string        # e.g. "gbe.tasks.email-send.queue"
    timestamp   : uint64        # unix millis, set by transport at publish time
    trace_id    : string?       # optional, propagated for observability

    payload     : bytes         # opaque to transport — domain-specific schema
}
```

Four fields plus payload. The transport is a dumb pipe.

**Removed fields and why**:
- ~~`domain`~~ — derivable from `subject` second token. Single source of truth.
- ~~`reply_to`~~ — request/reply is a non-goal. Add later if needed.
- ~~`schema_ver`~~ — payload concern, not transport concern. Lives in the
  payload schema itself (each domain schema carries its own version).
- ~~`content_type`~~ — transport doesn't interpret payload bytes. Whether
  the payload is JSON, protobuf, or a claim-check reference is between
  producer and consumer. Domain schemas define their own serialization.

### What the transport touches
- `message_id`: generated on publish
- `subject`: used for routing
- `timestamp`: set on publish

### What the transport ignores
- `payload`: opaque bytes, passed through untouched
- `trace_id`: passed through for observability propagation

### Wire format
The envelope itself is serialized as JSON (human-readable, debuggable).
The payload within it is opaque bytes — could be JSON, protobuf, msgpack,
or anything. The domain schema dictates serialization, not the transport.

---

## Payload Size and the Claim Check Pattern

### Transport payload limit
The transport enforces a `max_payload_size` (configured in `TransportConfig`,
default 1MB). Publishes exceeding this are rejected immediately — fail fast
rather than discovering broker limits at runtime.

```
TransportConfig {
    ...
    max_payload_size: 1_048_576   # 1MB default, matches NATS default
}
```

This is a **safety rail**, not a feature. The transport doesn't know *why*
a message is too big — it just prevents accidents.

### Claim check pattern (producer responsibility)
When a producer has data too large for a message (binary artifacts, large
reports, etc.), the producer stores the data in object storage and publishes
a reference:

```
Producer                        Object Store (S3)           Transport
   |                                |                          |
   |  PUT large binary              |                          |
   |------------------------------->|                          |
   |  <-- s3://bucket/key ---------|                          |
   |                                                           |
   |  publish({ ref: "s3://bucket/key", size: 52428800 })     |
   |---------------------------------------------------------->|
   |                                        (payload is ~60 bytes, not 50MB)
```

This is entirely the **producer's** concern. The transport doesn't know
or care whether the payload contains inline data or a storage reference.
The **domain schema** defines which fields are inline vs. references.

Consumer fetches the referenced data from storage as needed.

### Where this is decided

| Concern | Owner |
|---|---|
| "Is this too big for a message?" | Producer |
| "Store binary elsewhere, publish reference" | Producer |
| "Reject oversized publishes" | Transport (safety rail) |
| "Is this field inline data or a reference?" | Domain schema |
| "Fetch referenced data from storage" | Consumer |

---

## Core Interface

Language-neutral pseudocode. Each implementation (Rust/Go/Python) adapts
to idiomatic patterns (traits, interfaces, protocols).

### Transport

The top-level handle. Created once, shared across the application.

```
interface Transport {
    # Lifecycle
    connect(config: TransportConfig) -> Transport
    close()

    # Publishing
    publish(subject: string, payload: bytes, opts: PublishOpts?) -> MessageId

    # Subscribing
    subscribe(subject: string, group: string, handler: MessageHandler, opts: SubscribeOpts?) -> Subscription

    # Stream management
    ensure_stream(config: StreamConfig)

    # Health
    ping() -> bool
}
```

### Message (what the handler receives)

```
interface Message {
    envelope    : Envelope      # full envelope including metadata
    payload     : bytes         # raw payload bytes

    ack()                       # mark successfully processed
    nak(delay: duration?)       # reject, redeliver after optional delay
    dead_letter(reason: string) # send to dead-letter stream, ack original
}
```

### MessageHandler

```
callback MessageHandler(msg: Message) -> Result<void, Error>
```

If the handler returns an error and doesn't explicitly ack/nak, the
transport calls `nak()` automatically.

### Subscription

```
interface Subscription {
    unsubscribe()
    is_active() -> bool
}
```

---

## Publish Flow

```
caller                          transport                       wire
  |                                |                              |
  |  publish(subject, payload)     |                              |
  |------------------------------->|                              |
  |                                |  generate message_id         |
  |                                |  set timestamp               |
  |                                |  wrap in envelope            |
  |                                |  serialize                   |
  |                                |------------------------------->
  |                                |  Redis: XADD subject * ...  |
  |                                |  NATS:  js.Publish(subject)  |
  |  <-- message_id ---------------|                              |
```

### PublishOpts

```
PublishOpts {
    trace_id        : string?   # propagate from caller context
    idempotency_key : string?   # deduplicate on transport side if supported
}
```

Minimal — only fields the transport actually uses. Everything about
the payload (schema version, content type, serialization format) is
the domain publisher's concern, encoded within the payload bytes.

---

## Subscribe Flow

```
caller                          transport                       wire
  |                                |                              |
  |  subscribe(subj, group, fn)   |                              |
  |------------------------------->|                              |
  |                                |  Redis: XREADGROUP group ... |
  |                                |  NATS:  js.Subscribe(subj)   |
  |                                |                              |
  |                                |  <-- raw message ------------|
  |                                |  deserialize envelope        |
  |                                |  build Message               |
  |  <-- handler(msg) ------------|                              |
  |                                |                              |
  |  msg.ack()                     |                              |
  |------------------------------->|                              |
  |                                |  Redis: XACK stream group id |
  |                                |  NATS:  msg.Ack()            |
```

### SubscribeOpts

```
SubscribeOpts {
    batch_size  : uint32        # max messages per read (default 10)
    max_inflight: uint32        # max unacked messages (backpressure)
    ack_timeout : duration      # auto-nak if handler doesn't ack within this
    start_from  : StartPosition # "latest" | "earliest" | "timestamp(t)" | "id(x)"
}
```

---

## Stream Management

Streams must exist before subscribe (auto-created on publish for Redis,
pre-configured for NATS). `ensure_stream` is idempotent — safe to call
on every startup.

```
StreamConfig {
    subject     : string        # stream subject/key pattern
    max_age     : duration      # retention TTL (e.g. 72h, 7d)
    max_bytes   : uint64?       # optional size cap
    max_msgs    : uint64?       # optional message count cap
    storage     : "persistent" | "memory"   # default "persistent"
    replicas    : uint8         # NATS: replication factor; Redis: ignored (MemoryDB handles)
}
```

Called at application startup, driven by the task type config:

```yaml
streams:
  # notify domain
  - subject: "gbe.notify.topic.*"
    max_age: 72h

  - subject: "gbe.notify.broadcast.*"
    max_age: 7d

  # events domain
  - subject: "gbe.events.system.*"
    max_age: 7d

  - subject: "gbe.events.api.*"
    max_age: 3d

  - subject: "gbe.events.audit.*"
    max_age: 7d              # live window; archiver drains to cold storage

  # tasks domain (generated per task type from task_types config)
  - subject: "gbe.tasks.{task_type}.queue"
    max_age: 24h

  - subject: "gbe.tasks.{task_type}.progress"
    max_age: 24h

  - subject: "gbe.tasks.{task_type}.terminal"
    max_age: 24h

  # dead letter
  - subject: "gbe._deadletter.*"
    max_age: 7d
```

---

## Dead Letter Handling

When a consumer can't process a message (schema mismatch, corrupt payload,
repeated handler failures):

```
msg.dead_letter(reason: "schema validation failed: missing field task_id")
```

This:
1. Publishes the original envelope + reason to `gbe._deadletter.{domain}`
2. Acks the original message (removes it from the source stream)
3. Dead-letter stream has its own retention (7d default)

Dead-letter consumers are monitoring/alerting tools, not retry mechanisms.
Retries belong in the task layer, not the transport.

---

## Domain Publishers (Thin Wrappers)

Raw `transport.publish()` is low-level. Each domain provides a typed
publisher that knows its subject patterns, schema version, and serialization.
The transport just sees `publish(subject, bytes)`.

```
# Tasks domain publisher
struct TaskPublisher {
    transport: Transport

    fn submit_job(task_type: string, task_id: string, params: bytes) {
        subject = "gbe.tasks.{task_type}.queue"
        event = TaskQueueEvent {
            schema_ver: 1,          # payload-level, not transport-level
            task_id,
            state: "pending",
            params,
        }
        payload = validate_and_serialize(event)  # schema check + serialize
        transport.publish(subject, payload)
    }

    fn report_step(task_type: string, task_id: string, step: uint, result: bytes) {
        subject = "gbe.tasks.{task_type}.progress"
        event = TaskProgressEvent { schema_ver: 1, task_id, step, result }
        payload = validate_and_serialize(event)
        transport.publish(subject, payload)
    }

    fn complete_job(task_type: string, task_id: string, result: bytes) {
        subject = "gbe.tasks.{task_type}.terminal"
        event = TaskTerminalEvent { schema_ver: 1, task_id, state: "completed", result }
        payload = validate_and_serialize(event)
        transport.publish(subject, payload)
    }
}
```

The domain publisher owns: subject routing, schema version, validation,
serialization format. The transport owns: envelope wrapping, delivery,
persistence. Clean separation.

Same pattern for `NotifyPublisher` and `EventsPublisher`.

---

## Redis Streams Implementation Notes

### Mapping

| Transport concept | Redis implementation |
|---|---|
| `subject` | stream key (`:` delimited: `gbe:tasks:email-send:queue`) |
| `publish` | `XADD {stream} * {field} {value} ...` |
| `subscribe` | `XREADGROUP GROUP {group} {consumer} BLOCK {ms} COUNT {n} STREAMS {stream} >` |
| `ack` | `XACK {stream} {group} {id}` |
| `nak` | Don't ack — message redelivered on next `XREADGROUP` with `0` id claim |
| `nak(delay)` | `XACK` + re-publish with a scheduled visibility field (application-level) |
| `dead_letter` | `XADD gbe:_deadletter:{domain} * ...` + `XACK` original |
| `ensure_stream` | `XGROUP CREATE {stream} {group} $ MKSTREAM` (idempotent with try/catch) |
| `max_age` | Periodic `XTRIM` via sweeper (no native TTL on streams) |
| `message_id` | Generate ULID in application; store as field (Redis auto-id for stream ordering) |
| `backpressure` | Track pending count via `XPENDING`, pause reads if over `max_inflight` |

### Gotchas

- **No native stream TTL**: sweeper must run `XTRIM` periodically
- **Nak with delay**: Redis doesn't support delayed redelivery natively; options:
  - Claim-based: let `ack_timeout` expire, re-read via `XAUTOCLAIM`
  - Re-publish: ack original, publish new message with a "not before" field
- **Wildcard subscribe**: not supported on streams; application must manage explicit stream list

---

## NATS JetStream Implementation Notes

### Mapping

| Transport concept | NATS implementation |
|---|---|
| `subject` | NATS subject (`.` delimited, native) |
| `publish` | `js.Publish(subject, data)` |
| `subscribe` | `js.QueueSubscribe(subject, group, handler)` |
| `ack` | `msg.Ack()` |
| `nak` | `msg.Nak()` or `msg.NakWithDelay(d)` (native!) |
| `dead_letter` | Publish to `gbe._deadletter.{domain}` + `msg.Term()` |
| `ensure_stream` | `js.AddStream(StreamConfig{...})` (idempotent) |
| `max_age` | `StreamConfig.MaxAge` (native, per-stream) |
| `message_id` | `PublishOpts.MsgId` for deduplication; NATS also assigns sequence |
| `backpressure` | `SubscribeOpts.MaxAckPending` (native) |

### Advantages over Redis

- Native wildcard subscriptions (`gbe.tasks.*.terminal`)
- Native nak-with-delay
- Native per-stream retention (no sweeper needed)
- Native backpressure via `MaxAckPending`
- Subject hierarchy is first-class

---

## What the Transport Does NOT Do

- **Retry policy**: the task layer decides when/whether to retry
- **Job state**: lives in KV store, not in streams
- **Schema validation**: domain publishers validate before calling `transport.publish()`
- **Routing logic**: "which subject does this event go to?" is the domain publisher's job
- **Ordering guarantees across streams**: not provided; use KV state for cross-stream coordination

---

## Testing

### Interface compliance tests
Write transport-agnostic tests against the `Transport` interface:
- publish → subscribe → ack roundtrip
- nak → redelivery
- dead_letter → appears in dead-letter stream
- backpressure (max_inflight honored)
- ensure_stream idempotency

Run the same test suite against both Redis and NATS implementations.
This is the contract that guarantees swappability.

### In-memory transport for unit tests
Implement a minimal in-memory `Transport` for domain logic unit tests.
No Redis/NATS needed. Just a map of subjects → queues with ack tracking.

---

## File/Package Structure (proposed)

```
transport/
├── interface.go / mod.rs / __init__.py   # Transport trait/interface
├── envelope.go / mod.rs / envelope.py    # Envelope struct + serialization
├── redis/                                # Redis Streams implementation
│   ├── transport.go / mod.rs / ...
│   └── stream_manager.go / ...
├── nats/                                 # NATS JetStream implementation (future)
│   └── ...
├── memory/                               # In-memory implementation (tests)
│   └── ...
└── config.go / mod.rs / config.py        # StreamConfig, TransportConfig, etc.

domains/
├── tasks/
│   ├── publisher.go / mod.rs / ...       # TaskPublisher (typed, validated)
│   └── schemas.go / mod.rs / ...         # TaskQueueEvent, TaskProgressEvent, etc.
├── notify/
│   ├── publisher.go / mod.rs / ...
│   └── schemas.go / mod.rs / ...
└── events/
    ├── publisher.go / mod.rs / ...
    └── schemas.go / mod.rs / ...
```
