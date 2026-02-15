# Domain Schema Template

**Date**: 2026-02-14
**Context**: Minimal template for any project adding a domain to the
transport. The transport doesn't care about payload contents — this
template defines the contract between producers and consumers.

---

## Required Payload Fields

Every domain payload must include these fields. Beyond this, the domain
defines its own structure.

```
{
    "v":  1,                    # schema version (uint)
    "ts": 1707934567000,        # event timestamp, unix millis (uint64)
    "id": "evt_abc123"          # domain-level event ID (string)

    # ... domain-specific fields
}
```

**Why these three**:
- `v` — consumer knows how to deserialize; enables schema evolution
- `ts` — when the event occurred (may differ from envelope timestamp
  which is when it was published)
- `id` — deduplication key for consumers and archiver

**Naming**: short keys to keep payloads compact. `v` not `schema_version`.
`ts` not `timestamp`. The envelope already carries verbose metadata.

---

## Schema Evolution Rules

1. **Adding fields**: always safe — consumers ignore unknown fields
2. **Removing fields**: stop producing first, then remove from schema
   in the next version bump
3. **Changing field types**: never — add a new field instead
4. **Version bump**: increment `v` when removing fields or changing
   semantics of existing fields. Adding fields does not require a bump.

---

## Domain Registration

When a project adds a new domain, it needs:

1. **A subject prefix** under `gbe.{domain}.*`
2. **A schema definition** (language-native structs, protobuf, or JSON Schema)
3. **A domain publisher** that validates and serializes payloads
4. **A retention entry** in the sweeper config

That's it. No central registry to update. The transport routes by subject;
the schema lives with the producing project.

### Example: Adding a "billing" domain

```yaml
# retention config addition
"gbe.billing.*": 30d
```

```
# subject hierarchy
gbe.billing.invoice.created
gbe.billing.invoice.paid
gbe.billing.payment.failed
```

```
# payload example
{
    "v": 1,
    "ts": 1707934567000,
    "id": "evt_inv_001",
    "invoice_id": "inv_abc123",
    "amount_cents": 9900,
    "currency": "USD"
}
```

---

## Anti-Patterns

- **Don't nest the envelope fields in the payload** — `message_id` and
  envelope `timestamp` already exist in the envelope. The payload `ts`
  is event time, not publish time.
- **Don't use the payload for routing** — the transport routes on
  subject. If you need different consumers for different event types,
  use different subjects, not a `type` field that consumers filter on.
- **Don't put large data inline** — use claim check references for
  anything that might approach the transport's `max_payload_size`.
