# Redis Transport Implementation Plan

## Context
The core `Transport`, `Message`, `MessageHandler`, and `Subscription` traits are defined in `crates/transport/`. The `crates/transport-redis/` crate is a stub. This plan implements the Redis Streams backend, validating the trait design against real infrastructure.

## Module Structure
```
crates/transport-redis/src/
  lib.rs          -- re-exports RedisTransport + RedisTransportConfig
  config.rs       -- RedisTransportConfig
  transport.rs    -- RedisTransport (impl Transport)
  message.rs      -- RedisMessage (impl Message)
  subscription.rs -- RedisSubscription (impl Subscription)
  consumer.rs     -- background XREADGROUP loop
  subject.rs      -- subject_to_key, extract_domain helpers
  error.rs        -- redis::RedisError -> TransportError mapping
```

## Key Structs

### RedisTransport
- Holds `ConnectionManager` (auto-reconnect, Clone/Arc-based) + config
- `connect(config)` opens the connection
- `publish`: build Envelope, check payload size, serialize to JSON, `XADD {key} * envelope {json}`
- `subscribe`: generate consumer ID (`{hostname}-{ulid}`), spawn tokio task, return `RedisSubscription`
- `ensure_stream`: `XGROUP CREATE {key} _init $ MKSTREAM`, catch BUSYGROUP
- `ping`: `redis::cmd("PING")`
- `close`: set `closed` AtomicBool flag, checked by publish/subscribe

### RedisMessage
- Holds: `Envelope`, stream_key, group, entry_id, cloned `ConnectionManager`, `AtomicBool` acked flag
- `ack()`: `XACK`, set acked=true
- `nak(delay)`: no-op (claim-based — message stays in PEL, reclaimed after ack_timeout)
- `dead_letter(reason)`: `XADD gbe:_deadletter:{domain}`, then `XACK` original

### RedisSubscription
- Holds: `CancellationToken`, `Arc<AtomicBool>` active flag, `JoinHandle`
- `unsubscribe()`: cancel token
- `is_active()`: read flag

### Consumer Loop (two-phase)
1. **Reclaim phase** (periodic): `XAUTOCLAIM` to pick up timed-out messages from PEL
2. **Read phase**: `XREADGROUP ... STREAMS {key} >` for new messages
3. Deserialize envelope, build `RedisMessage`, call handler
4. On handler error + not explicitly acked: do nothing (claim-based nak)
5. Backpressure: check `XPENDING` count, sleep if >= max_inflight

### Subject Mapping
- `subject_to_key`: replace `.` with `:` (`gbe.tasks.email-send.queue` → `gbe:tasks:email-send:queue`)
- `extract_domain`: second token after split on `:`

## Dependency Changes

**`Cargo.toml` (workspace):** add `tokio-util`, `tracing`
**`crates/transport-redis/Cargo.toml`:** add `tokio-util` (CancellationToken), `tracing`, `hostname`, `ulid`

## Implementation Order
1. `subject.rs` + `error.rs` — pure functions
2. `config.rs` — struct
3. `message.rs` — RedisMessage + Message impl
4. `subscription.rs` — RedisSubscription + Subscription impl
5. `consumer.rs` — the consumer loop
6. `transport.rs` — RedisTransport + Transport impl
7. `lib.rs` — re-exports
8. Unit tests for subject/error helpers
9. Integration test (guarded by REDIS_URL env var)

## Testing
- Unit tests: subject mapping, error mapping, envelope round-trip
- Integration tests in `crates/transport-redis/tests/`: publish→subscribe roundtrip, dead letter routing, ensure_stream idempotency
- Guard integration tests with `REDIS_URL` env var so CI skips without Redis

## Verification
```bash
cargo fmt --check
cargo clippy --workspace
cargo test --workspace                    # unit tests
REDIS_URL=redis://localhost:6379 cargo test --workspace  # with Redis
```
