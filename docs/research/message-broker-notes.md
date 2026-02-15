# Message Broker & Event Bus Research Notes

**Date**: 2026-02-14
**Context**: Designing the messaging layer for gbe-notify (ntfy-compatible backend)

---

## Use Cases

1. **Customer notifications** — subscribers receive push notifications (ntfy-compatible API)
2. **Observability events** — system-generated events for monitoring
3. **Task orchestration** — triggers, job steps, workflow coordination

## Architecture Decision: Layered Approach

```
┌─────────────────────────────────┐
│  Task System (our code)         │  ← state machines, retries,
│  - job lifecycle                │    step sequencing, timeouts
│  - step orchestration           │
│  - timeout/retry policy         │
├─────────────────────────────────┤
│  Message Transport              │  ← delivery, routing,
│  (NATS JetStream or Redis)     │    persistence, fan-out
│  - publish/subscribe            │
│  - consumer groups              │
│  - ack/nak/redelivery           │
└─────────────────────────────────┘
```

**Key principle**: Transport handles **delivery**. Task layer handles **semantics**.

### Transport Should NOT Handle
- Retry logic (conflates "unacked message" with "task failed")
- Job state (use a separate state store)

### Transport SHOULD Handle
- Message routing and fan-out
- Persistence and replay
- Consumer groups and ack/nak
- Backpressure

### Task State Pattern
- State lives in KV store (NATS KV or Redis hash)
- Messages are just signals between steps
- Sweeper process re-publishes stuck jobs from state store

```
Job submitted → publish to `tasks.{type}.pending`
Worker picks up → ack message, set state to "running"
Step completes → publish to `tasks.{type}.step.{n}.complete`
Next worker picks up → repeat
Job fails → publish to `tasks.{type}.failed`, update state
Timeout? → Sweeper reads state store, re-publishes stuck jobs
```

---

## Broker Comparison

### Evaluated and Rejected
- **Apache Kafka** — operationally heavy, overkill for our scale, strong human preference against it
- **Temporal** — good for task orchestration only, doesn't solve notifications or observability

### Current: AWS MemoryDB (Redis-compatible)
- Already in use for pub/sub
- Redis Streams available for consumer groups + persistence
- **Limitation**: basic pub/sub loses messages if no subscriber is connected
- **Redis Streams**: adds durability, consumer groups, ack — addresses the gap

### Candidate: NATS JetStream
- Single Go binary, trivial to deploy
- Three primitives: Core NATS (fire-and-forget pub/sub), JetStream (persistent streams), KV/Object Store
- Subject hierarchy with wildcards: `notifications.customer.>`, `events.observability.api.latency`
- Built-in clustering, exactly-once semantics
- Persistence: file-based or memory-based (configurable per stream)
- **Persistence options**:
  - File store (default) — data written to disk in NATS data directory
  - Memory store — faster but volatile
  - Both are per-stream configurable
  - No external database required

### RabbitMQ
- Mature, flexible routing (exchanges/bindings)
- Good for task queues
- Not ideal for high-throughput event streaming
- More operational complexity than NATS

### Redis Streams (via MemoryDB)
- Already available — no new infrastructure
- Consumer groups with acknowledgment
- Persistence handled by MemoryDB (Multi-AZ replication)
- Weaker replay compared to NATS/Kafka
- Good enough for our scale

---

## AWS MemoryDB Notes

- Redis-compatible (Redis 7.x API)
- **Supports Redis Streams** — consumer groups, XADD, XREADGROUP, XACK all work
- Multi-AZ with automatic failover
- Persistent (transaction log replicated across AZs)
- Unlike ElastiCache, MemoryDB is durable by default
- Already in our infrastructure — zero new operational cost

---

## NATS JetStream Persistence Details

- **No external database** — persistence is built-in
- File store: writes to local disk (or EBS/EFS on AWS)
- Configurable per stream:
  - `max_msgs` — max messages retained
  - `max_bytes` — max storage size
  - `max_age` — message TTL
  - `storage` — `file` or `memory`
- Clustering: Raft-based consensus, data replicated across nodes
- On AWS: run NATS on EC2/ECS with EBS volumes, or use Synadia Cloud (managed NATS)

---

## Recommendation

**Short-term**: Use MemoryDB with Redis Streams (already have it, no new infra).
**Evaluate**: NATS JetStream if we outgrow Redis or want cleaner subject hierarchy.
**The transport is swappable** if we keep the abstraction boundary clean.

---

## Decisions

### Terminology
- **Domain**: the top-level schema category — `notify`, `events`, `tasks`
- Each domain defines its own schema contract
- Transport carries an **envelope** (routing metadata); the **payload** conforms to the domain schema
- Transport never inspects the payload; schema validation is producer/consumer responsibility

### Customer Notifications (Q2)
- < 10k customers — per-customer streams are fine, no hash-bucketing needed
- ntfy usage is internal devops, not customer-facing
- Simplifies the notify domain: `gbe.notify.topic.*` is the primary path

### Event Retention (Q3)
| Domain | TTL | Notes |
|---|---|---|
| `notify.topic.*` | 72h | Internal devops, short-lived |
| `notify.broadcast.*` | 7d | Maintenance notices |
| `events.system.*` | 7d | Incident debugging window |
| `events.api.*` | 3d | High volume, short relevance |
| `events.audit.*` | 7d in-stream | Live query window |
| `events.audit.*` (archive) | 365d+ in cold storage | S3/Postgres via archive sweeper |
| `tasks.*` | 24h | State lives in KV; stream is just signals |

**Archive sweeper pattern**:
- Sweeper consumes from streams, knows per-domain retention policy
- Ephemeral domains: drop after TTL (XTRIM)
- Audit domain: flush to cold storage (S3 + Athena or Postgres), then trim stream
- Sweeper confirms successful archive before trimming

### Schemas (Q4)
- **No schema registry service** — shared type definitions in code
- Languages: Rust, Go, Python (no JavaScript)
- Schema definitions in a structured format (protobuf, JSON Schema, or language-native structs)
- Shared schema package per language, strict contracts for producers and consumers
- **Transport envelope / payload separation**:
  ```
  ┌─ Transport Envelope ──────────────┐
  │  subject/stream, timestamp, id    │  ← transport routes on this
  │  ┌─ Payload (domain schema) ────┐ │
  │  │  domain-specific fields      │ │  ← producers/consumers validate this
  │  └──────────────────────────────┘ │
  └────────────────────────────────────┘
  ```
- **Evolution rules**: add fields freely, never change field types, deprecate before removing
- **Validation**: at producer (before publish) and consumer (after receive)
- **Dead-letter stream**: malformed messages go to `gbe._deadletter.{domain}` for debugging

### Task Stream Granularity (Q1)
**Resolved**: Option C — hybrid split by consumer role. 3 streams per task type: `queue` (workers), `progress` (orchestrator), `terminal` (monitors). 50 task types × 3 = ~150 streams. Peak 10k msgs/min on hot types. Config-driven onboarding. See [subject-hierarchy.md](./subject-hierarchy.md) for full details.

---

## Subject/Topic Hierarchy

See: [subject-hierarchy.md](./subject-hierarchy.md)

## Transport Abstraction Layer

See: [transport-abstraction.md](./transport-abstraction.md)

## KV State Store

See: [kv-state-store.md](./kv-state-store.md)

## Sweeper & Archiver

See: [sweeper-archiver.md](./sweeper-archiver.md)

## Domain Schema Template

See: [domain-schema-template.md](./domain-schema-template.md)
