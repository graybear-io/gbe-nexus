# Subject / Topic Hierarchy

**Date**: 2026-02-14
**Context**: Applies whether transport is Redis Streams or NATS JetStream.
For Redis, these are logical channel/stream names. For NATS, these are native subjects with wildcard routing.

---

## Design Principles

1. **Dot-delimited** — works natively with NATS wildcards, easy to parse for Redis
2. **Pattern**: `{domain}.{entity}.{action/qualifier}`
3. **Wildcards** (NATS): `*` matches one token, `>` matches one or more trailing tokens
4. **For Redis**: implement wildcard matching in application code or use separate streams per leaf

---

## Hierarchy

```
gbe.                                    # root namespace
├── notify.                             # ── customer-facing notifications ──
│   ├── customer.{customer_id}.>        # all notifications for a customer
│   │   ├── ...alert                    # urgent/actionable notifications
│   │   ├── ...info                     # informational
│   │   └── ...digest                   # batched/summary notifications
│   ├── broadcast.>                     # all-customer broadcasts
│   │   ├── ...maintenance              # planned maintenance
│   │   └── ...announcement             # product announcements
│   └── topic.{topic_name}             # ntfy-compatible topic (flat, maps to ntfy API)
│
├── events.                             # ── observability ──
│   ├── system.>                        # infrastructure events
│   │   ├── ...health                   # health checks, heartbeats
│   │   ├── ...deploy                   # deployment events
│   │   └── ...error                    # system errors
│   ├── api.>                           # API-layer events
│   │   ├── ...request                  # request logs (sampled)
│   │   └── ...error                    # API errors
│   ├── auth.>                          # authentication events
│   │   ├── ...login                    # login attempts
│   │   ├── ...logout                   # logouts
│   │   └── ...failure                  # auth failures
│   └── audit.>                         # audit trail
│       ├── ...change                   # data mutations
│       └── ...access                   # sensitive data access
│
└── tasks.                              # ── task orchestration ──
    ├── {task_type}.                    # per task type, 3 streams by consumer role
    │   ├── queue                       # pending + claimed (workers)
    │   ├── progress                    # step start/complete/fail (orchestrator)
    │   └── terminal                    # completed + failed + cancelled (monitors)
    └── _control.>                      # task system control plane
        ├── ...sweep                    # sweeper found stuck jobs
        ├── ...rebalance                # worker rebalancing
        └── ...shutdown                 # graceful shutdown signal
```

---

## Examples

### Customer Notifications
```
gbe.notify.customer.cust_abc123.alert       # urgent alert for customer abc123
gbe.notify.customer.cust_abc123.info        # info notification
gbe.notify.broadcast.maintenance            # maintenance notice to all
gbe.notify.topic.server-alerts              # ntfy-compatible topic
```

### Subscribing (NATS wildcards)
```
gbe.notify.customer.cust_abc123.>           # everything for one customer
gbe.notify.customer.*.alert                 # all customer alerts
gbe.notify.broadcast.>                      # all broadcasts
gbe.events.>                                # all observability events
gbe.tasks.email-send.>                      # all email-send task events
```

### Observability
```
gbe.events.system.health                    # health heartbeat
gbe.events.api.error                        # API error occurred
gbe.events.auth.failure                     # failed login attempt
gbe.events.audit.change                     # data was mutated
```

### Task Orchestration
```
gbe.tasks.email-send.queue                  # worker picks up pending/claimed jobs
gbe.tasks.email-send.progress               # orchestrator sees step events
gbe.tasks.email-send.terminal               # monitors see completed/failed/cancelled
gbe.tasks.report-gen.terminal               # report-gen outcomes
gbe.tasks._control.sweep                    # sweeper heartbeat
```

---

## ntfy Compatibility Bridge

The ntfy API uses flat topic names (`/mytopic`). Map them into the hierarchy:

```
ntfy topic "server-alerts"  →  gbe.notify.topic.server-alerts
ntfy topic "backups"        →  gbe.notify.topic.backups
```

The HTTP layer translates:
- `POST /server-alerts` → publish to `gbe.notify.topic.server-alerts`
- `GET /server-alerts/json` → subscribe to `gbe.notify.topic.server-alerts`

This keeps ntfy API compatibility without polluting the broader namespace.

---

## Redis Stream Mapping

If using Redis Streams instead of NATS, map subjects to stream keys:

```
NATS subject:    gbe.notify.customer.cust_abc123.alert
Redis stream:    gbe:notify:customer:cust_abc123:alert

NATS subject:    gbe.tasks.email-send.pending
Redis stream:    gbe:tasks:email-send:pending
```

Use `:` delimiter (Redis convention) instead of `.`.

Wildcard subscriptions require application-level fan-out or pattern-based PSUBSCRIBE on a parallel pub/sub channel that mirrors stream writes.

---

## Resolved Decisions

- **Q2 — Customer partitioning**: < 10k customers, per-customer streams are fine. ntfy is internal devops use only, so `gbe.notify.topic.*` is the primary path.
- **Q3 — Event retention**: Per-domain TTLs enforced by archive sweeper. See [message-broker-notes.md](./message-broker-notes.md) for full retention table. Audit drains to cold storage before trim.
- **Q4 — Schemas**: Shared type definitions per domain (Rust/Go/Python), no registry service. Transport envelope wraps domain-specific payload. Validation at producer and consumer boundaries. Dead-letter stream for malformed messages.

---

## Decision: Task Stream Granularity (Q1)

**Chosen: Option C — Hybrid, split by consumer role**

### Context
- Consolidating dozens of ad-hoc task/event flows into unified system
- 30-50 task types expected
- Peak volume: ~10k messages/min on hot task types
- At that volume, Option A forces workers to filter ~99% irrelevant events

### Stream Layout (3 streams per task type)

```
gbe.tasks.{task_type}.queue       → pending, claimed      (worker-facing)
gbe.tasks.{task_type}.progress    → step start/complete/fail (orchestrator-facing)
gbe.tasks.{task_type}.terminal    → completed, failed, cancelled (monitors/dashboards)
gbe.tasks._control.>              → sweep, rebalance, shutdown (system-wide)
```

### Scale
- 50 task types × 3 streams = 150 task streams + control streams
- MemoryDB handles this trivially (~167 ops/sec per hot stream at 10k/min)
- Redis `XADD` auto-creates streams on first write — no pre-provisioning

### Consumer Groups

| Stream | Consumer Group | Purpose |
|---|---|---|
| `*.queue` | `{task_type}-workers` | Workers competing for jobs |
| `*.progress` | `{task_type}-orchestrator` | Step coordinator advancing jobs |
| `*.terminal` | `monitors` | Dashboards, alerting, sweeper |
| `*.terminal` | `archiver` | Archive sweeper for cold storage drain |

### Onboarding New Task Types

Driven by config, not code:

```yaml
task_types:
  email-send:
    workers: 4
    step_timeout: 30s
  report-gen:
    workers: 2
    step_timeout: 300s
  deploy-pipeline:
    workers: 1
    step_timeout: 600s
```

Adding a task type = adding a config entry. Streams and consumer groups are created on first use.

### Full Job History
Streams are **signals**, not the system of record. Full job state lives in KV store (Redis hash or NATS KV), keyed by `task_id`. The sweeper and any "show me job X history" queries read from KV, not from streams.
