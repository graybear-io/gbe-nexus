# ntfy Android App API Compatibility Schema

**Document Purpose**: Complete API schema and endpoint documentation for maintaining Android app compatibility when rebuilding the ntfy backend with a different message store/pub-sub system.

**Source Analysis Date**: 2026-02-14
**ntfy Version**: Based on current main branch
**Reference Files**:
- `server/types.go:28-99` (message schema)
- `server/server.go:568-580` (endpoints)
- `docs/subscribe/api.md` (subscription API)
- `docs/publish.md` (publish API)

---

## 1. Core Message Schema

### 1.1 Message Object (JSON)

**Location**: `server/types.go:28-48`

```json
{
  "id": "string",                    // Required: 12-char random ID
  "sequence_id": "string",           // Optional: For message updates (omitted if same as id)
  "time": 1234567890,                // Required: Unix timestamp (seconds)
  "expires": 1234567890,             // Optional: Unix timestamp (seconds)
  "event": "message",                // Required: See Event Types below
  "topic": "mytopic",                // Required: Topic name
  "title": "string",                 // Optional: Notification title
  "message": "string",               // Optional: Notification body
  "priority": 3,                     // Optional: 1-5 (default: 3)
  "tags": ["tag1", "warning"],       // Optional: Array of strings
  "click": "https://example.com",    // Optional: URL to open on click
  "icon": "https://example.com/i.png", // Optional: Icon URL
  "actions": [...],                  // Optional: See Actions schema
  "attachment": {...},               // Optional: See Attachment schema
  "poll_id": "string",               // Optional: For poll request events
  "content_type": "text/plain",      // Optional: "text/plain" or "text/markdown"
  "encoding": "base64"               // Optional: Empty (UTF-8) or "base64"
}
```

### 1.2 Event Types

**Location**: `server/types.go:14-21`

| Event | Description | When Sent |
|-------|-------------|-----------|
| `open` | Connection opened | First message on new subscription |
| `keepalive` | Connection keepalive | Periodically during subscription |
| `message` | Actual notification | When message published |
| `message_delete` | Delete a message | When message deleted by sequence_id |
| `message_clear` | Clear all messages | When topic cleared |
| `poll_request` | Poll request from iOS | Special iOS polling mechanism |

**Android App Note**: Android app primarily handles `message` events. `open` and `keepalive` maintain the connection.

### 1.3 Actions Schema

**Location**: `server/types.go:87-99`

Actions enable interactive buttons in notifications:

```json
{
  "id": "action1",                   // Required: Unique action ID
  "action": "http",                  // Required: "view", "broadcast", "http", "copy"
  "label": "Open Door",              // Required: Button text
  "clear": true,                     // Optional: Clear notification after action
  "url": "https://api.example.com",  // For "view" and "http" actions
  "method": "POST",                  // For "http" action (default: POST)
  "headers": {                       // For "http" action
    "Authorization": "Bearer token"
  },
  "body": "payload",                 // For "http" action
  "intent": "com.example.ACTION",    // For "broadcast" action (Android)
  "extras": {                        // For "broadcast" action (Android)
    "key": "value"
  },
  "value": "text to copy"            // For "copy" action
}
```

**Action Types**:
- `view`: Open URL in browser
- `http`: Send HTTP request
- `broadcast`: Send Android broadcast intent
- `copy`: Copy text to clipboard

### 1.4 Attachment Schema

**Location**: `server/types.go:79-85`

```json
{
  "name": "document.pdf",            // Required: Filename
  "type": "application/pdf",         // Optional: MIME type
  "size": 123456,                    // Optional: Size in bytes
  "expires": 1234567890,             // Optional: Unix timestamp
  "url": "https://example.com/file"  // Required: Download URL
}
```

---

## 2. API Endpoints

### 2.1 Publish Endpoint

**Method**: `POST` or `PUT`
**Path**: `/<topic>`
**Content-Type**: `text/plain` (or `application/json` for structured publish)

#### Request Headers (all optional)

| Header | Alternative Names | Description | Example |
|--------|------------------|-------------|---------|
| `Title` | `X-Title`, `t` | Notification title | `Title: Server Alert` |
| `Message` | `X-Message`, `m` | Message body (alt to body) | `Message: Disk full` |
| `Priority` | `X-Priority`, `prio`, `p` | 1=min, 2=low, 3=default, 4=high, 5=max/urgent | `Priority: urgent` |
| `Tags` | `X-Tags`, `tag`, `ta` | Comma-separated tags/emojis | `Tags: warning,skull` |
| `Delay` | `X-Delay` | Delay delivery | `Delay: 30min` |
| `Click` | `X-Click` | URL to open on click | `Click: https://example.com` |
| `Icon` | `X-Icon` | Icon URL | `Icon: https://example.com/icon.png` |
| `Attach` | `X-Attach`, `a` | Attachment URL | `Attach: https://example.com/file.pdf` |
| `Filename` | `X-Filename`, `file`, `f` | Attachment filename override | `Filename: backup.tar.gz` |
| `Actions` | `X-Actions` | Action buttons (see Actions Schema) | `Actions: view, Open, https://example.com` |
| `Email` | `X-Email`, `e-mail`, `mail`, `e` | Forward to email | `Email: user@example.com` |
| `Call` | `X-Call`, `phone` | Make phone call | `Call: +1234567890` |
| `Cache` | `X-Cache` | Cache message (yes/no) | `Cache: no` |
| `Firebase` | `X-Firebase` | Send to Firebase (yes/no) | `Firebase: yes` |
| `UnifiedPush` | `X-UnifiedPush`, `up` | UnifiedPush message | `UnifiedPush: 1` |
| `Markdown` | `X-Markdown` | Enable markdown | `Markdown: yes` |
| `Sequence-Id` | `X-Sequence-Id`, `sid` | Message sequence ID for updates | `Sequence-Id: msg001` |

**Priority Values**:
- `1` or `min`: Minimum priority
- `2` or `low`: Low priority
- `3` or `default`: Default (omitted = 3)
- `4` or `high`: High priority
- `5` or `max` or `urgent`: Maximum/urgent priority

#### Request Body

Simple text message or empty (if using `Message` header).

#### Response (JSON)

**Location**: Returns the created message object (same schema as subscription messages).

```json
{
  "id": "hwQ2YpKdmg",
  "time": 1635528741,
  "expires": 1635615141,
  "event": "message",
  "topic": "mytopic",
  "message": "Backup successful"
}
```

**Status Codes**:
- `200 OK`: Message published successfully
- `400 Bad Request`: Invalid parameters
- `429 Too Many Requests`: Rate limit exceeded
- `507 Insufficient Storage`: UnifiedPush topic without rate visitor (Android-specific)

---

### 2.2 Subscribe Endpoints

All subscription endpoints use long-lived HTTP connections (streaming).

#### 2.2.1 JSON Stream (Recommended for Android)

**Method**: `GET`
**Path**: `/<topic>/json` or `/<topic1>,<topic2>/json` (multi-topic)
**Content-Type**: `application/x-ndjson; charset=utf-8`
**Transfer-Encoding**: `chunked`

**Response**: Newline-delimited JSON (NDJSON), one message object per line.

**Example**:
```
GET /mytopic/json HTTP/1.1
Host: ntfy.sh

HTTP/1.1 200 OK
Content-Type: application/x-ndjson; charset=utf-8

{"id":"SLiKI64DOt","time":1635528757,"event":"open","topic":"mytopic"}
{"id":"hwQ2YpKdmg","time":1635528741,"event":"message","topic":"mytopic","message":"Disk full"}
{"id":"DGUDShMCsc","time":1635528787,"event":"keepalive","topic":"mytopic"}
```

#### 2.2.2 Server-Sent Events (SSE)

**Method**: `GET`
**Path**: `/<topic>/sse`
**Content-Type**: `text/event-stream; charset=utf-8`

**Response**: SSE format with `event:` and `data:` lines.

**Example**:
```
event: open
data: {"id":"weSj9RtNkj","time":1635528898,"event":"open","topic":"mytopic"}

data: {"id":"p0M5y6gcCY","time":1635528909,"event":"message","topic":"mytopic","message":"Hi!"}

event: keepalive
data: {"id":"VNxNIg5fpt","time":1635528928,"event":"keepalive","topic":"test"}
```

#### 2.2.3 Raw Stream

**Method**: `GET`
**Path**: `/<topic>/raw`
**Content-Type**: `text/plain; charset=utf-8`

**Response**: One line per message (message body only, no metadata).

#### 2.2.4 WebSocket

**Method**: `GET` (with `Upgrade: websocket`)
**Path**: `/<topic>/ws`

**Response**: WebSocket connection sending JSON message objects.

### 2.3 Query Parameters (Subscription)

**Location**: `server/server.go:1684-1696`

| Parameter | Alternative Names | Description | Example |
|-----------|------------------|-------------|---------|
| `poll` | `x-poll`, `po` | Poll mode (don't keep connection open) | `?poll=1` |
| `since` | `x-since` | Return messages since time/ID | `?since=1635528757` |
| `scheduled` | `x-scheduled`, `sched` | Include scheduled messages | `?scheduled=1` |
| `id` | `x-id` | Filter by message ID | `?id=hwQ2YpKdmg` |
| `message` | `x-message`, `m` | Filter by message text | `?message=backup` |
| `title` | `x-title`, `t` | Filter by title | `?title=Alert` |
| `priority` | `x-priority`, `p` | Filter by priority (comma-separated) | `?priority=4,5` |
| `tags` | `x-tags`, `tag`, `ta` | Filter by tags (comma-separated) | `?tags=warning` |

**Since Parameter Values**:
- Unix timestamp: `?since=1635528757`
- Message ID: `?since=hwQ2YpKdmg`
- `all`: All cached messages
- `none`: No cached messages (default)
- `latest`: Only the latest message

---

## 3. Topic Naming

**Regex**: `^[-_A-Za-z0-9]{1,64}$`
**Location**: `server/server.go:74`

**Rules**:
- 1-64 characters
- Alphanumeric, hyphens, underscores only
- No slashes (path separator)
- Case-sensitive

**Reserved Topics** (if changed, update Android app):
- `~control`: Firebase control topic (`server/server.go:141`)
- `~poll`: iOS polling topic (currently disabled)

**UnifiedPush Convention**:
- Topics starting with `up` (14 chars total) are rate-limited by subscriber
- Example: `up1234567890ab`

---

## 4. Authentication & Authorization

**Headers**:
- `Authorization: Bearer <token>` - Token-based auth
- `Authorization: Basic <base64>` - Basic auth (username:password)

**Endpoints**:
- `GET /<topic>/auth` - Test auth (returns `{"success": true}`)
- `POST /v1/account/token` - Issue token
- `GET /v1/account` - Get account info

**Note**: ntfy supports optional authentication. If auth is disabled, topics are "open" and topic name serves as password.

---

## 5. Firebase Integration

**Android App Note**: The Android app uses **Firebase Cloud Messaging (FCM)** for push notifications when the app is in background.

**Key Points**:
- When `Firebase: yes` header set, server sends to FCM topic
- Firebase topic format: same as ntfy topic
- Message truncated to 4000 chars for FCM
- FCM enables instant delivery without persistent connection
- Control topic `~control` used for special FCM messages

**Implications for Rebuild**:
- If maintaining Android app compatibility, must implement FCM sending
- Or accept that app requires persistent connection (battery drain)
- Or modify Android app to use different push mechanism

---

## 6. Additional Endpoints (Context)

These are not required for basic Android app functionality but provide full compatibility:

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/v1/health` | GET | Health check: `{"healthy": true}` |
| `/v1/version` | GET | Server version info |
| `/v1/config` | GET | Server configuration |
| `/v1/stats` | GET | Server statistics |
| `/<topic>/<sequence_id>` | PUT/PATCH | Update message by sequence_id |
| `/<topic>/<sequence_id>/read` | PUT | Mark message as read |
| `/<topic>/<sequence_id>/clear` | DELETE | Delete message by sequence_id |

---

## 7. Message Persistence (SQLite Cache)

**Location**: `server/message_cache.go`

**Schema**: Messages stored in SQLite with indexes on:
- `mid` (message ID)
- `sequence_id`
- `time`
- `topic`
- `expires`
- `sender`
- `user`

**Implications**:
- Your message store must support:
  - Time-based queries (`since` parameter)
  - ID-based queries
  - Sequence ID lookups (for updates/deletes)
  - Expiration/TTL
  - Multi-topic queries

---

## 8. Notes for Future Planning Session

### 8.1 Architecture Decisions

**Current ntfy Architecture**:
- ✅ **Pro**: Simple, no external dependencies
- ✅ **Pro**: Low latency (in-memory pub-sub)
- ❌ **Con**: Single-server only (no clustering)
- ❌ **Con**: Lost messages on restart if cache disabled
- ❌ **Con**: Topic subscribers don't persist across server restarts

**Questions for Rebuild**:
1. **Scale Requirements**: How many concurrent subscribers? Topics?
2. **HA/Clustering**: Need multi-server deployment?
3. **Persistence**: Always persist messages, or optional like ntfy?
4. **Message Store Options**:
   - PostgreSQL with LISTEN/NOTIFY
   - Redis Pub/Sub + Redis Streams (persistence)
   - NATS JetStream
   - Kafka (overkill for notifications)
   - MongoDB change streams

### 8.2 API Compatibility Strategy

**Option A: Full Compatibility**
- Implement exact API as documented here
- Android app works unchanged
- Must implement Firebase integration
- Must support all headers/parameters

**Option B: Adapter Layer**
- Build your backend with different API
- Create ntfy-compatible adapter/proxy
- Adapter translates between APIs
- Easier to evolve your API separately

**Option C: Fork Android App**
- Modify Android app to use your API
- More flexibility in backend design
- Maintenance burden of Android app fork

### 8.3 Critical Compatibility Points

**Must Implement**:
1. ✅ `GET /<topic>/json` - NDJSON streaming
2. ✅ `POST /<topic>` - Publish with headers
3. ✅ Message schema (id, time, event, topic, message)
4. ✅ Event types: open, keepalive, message
5. ✅ Priority levels (1-5)
6. ✅ `since` query parameter

**Optional but Recommended**:
- Actions schema (interactive buttons)
- Attachments
- Message updates (sequence_id)
- WebSocket endpoint
- Firebase integration (for background delivery)

**Can Skip**:
- SSE endpoint (if Android app only uses JSON)
- Raw endpoint (simplified format)
- Email/Call integration
- Scheduled messages
- UnifiedPush support

### 8.4 Testing Strategy

**Android App Compatibility Testing**:
1. Set up test server with your backend
2. Point Android app to your server (change base URL)
3. Test critical flows:
   - Subscribe to topic → receive `open` event
   - Publish message → receive `message` event
   - Keepalives arrive periodically
   - Priority levels display correctly
   - Actions work (if implemented)
   - App backgrounded → Firebase delivery (if implemented)

### 8.5 Message Store Requirements

Your message store must support:

| Feature | Requirement | Example |
|---------|-------------|---------|
| **Time-ordered reads** | Messages sorted by timestamp | For `since` parameter |
| **TTL/Expiration** | Auto-delete old messages | Based on `expires` field |
| **Atomic ID generation** | Unique message IDs | 12-char random string |
| **Multi-topic queries** | Query multiple topics at once | For `/<topic1>,<topic2>/json` |
| **Filtering** | By priority, tags, title | For query parameters |
| **Updates** | Update message by sequence_id | For message editing |
| **Deletes** | Delete by sequence_id | For message deletion |

**Recommended Approach**:
- **PostgreSQL**: Best for traditional DB setup
  - Use `LISTEN/NOTIFY` for real-time pub-sub
  - Store messages in table with indexes
  - Support all query patterns
- **Redis**: Best for high-throughput, ephemeral
  - Redis Streams for message log
  - Pub/Sub for real-time delivery
  - TTL built-in
- **NATS JetStream**: Best for distributed, cloud-native
  - Message streaming with persistence
  - Built-in clustering
  - May need adapter for HTTP API

### 8.6 Rate Limiting

**Location**: `server/visitor.go`

ntfy implements per-IP and per-user rate limiting:
- Messages per time window
- Subscriptions per IP
- Email/Call limits

**Your Implementation**:
- Decide if rate limiting needed
- If yes, track by IP or user ID
- Android app expects `429 Too Many Requests` on rate limit

### 8.7 Content-Type Handling

**Critical**: Android app expects:
- Subscribe responses: `application/x-ndjson; charset=utf-8`
- Publish responses: `application/json`
- Transfer-Encoding: `chunked` for streaming

### 8.8 CORS Headers

For web app compatibility:
```
Access-Control-Allow-Origin: *
Access-Control-Allow-Methods: GET, PUT, POST, DELETE
Access-Control-Allow-Headers: Authorization, Content-Type, ...
```

---

## 9. Example Message Flows

### 9.1 Simple Notification

**Publish**:
```http
POST /mytopic HTTP/1.1
Host: ntfy.sh
Title: Backup Complete
Priority: high
Tags: white_check_mark

Backup of server01 completed successfully
```

**Subscribe receives**:
```json
{
  "id": "abc123def456",
  "time": 1708012345,
  "expires": 1708098745,
  "event": "message",
  "topic": "mytopic",
  "title": "Backup Complete",
  "message": "Backup of server01 completed successfully",
  "priority": 4,
  "tags": ["white_check_mark"]
}
```

### 9.2 With Actions

**Publish**:
```http
POST /alerts HTTP/1.1
Actions: http, Acknowledge, https://api.example.com/ack/123, method=POST

Server disk usage critical
```

**Subscribe receives**:
```json
{
  "id": "xyz789abc012",
  "time": 1708012345,
  "event": "message",
  "topic": "alerts",
  "message": "Server disk usage critical",
  "actions": [
    {
      "id": "action1",
      "action": "http",
      "label": "Acknowledge",
      "url": "https://api.example.com/ack/123",
      "method": "POST"
    }
  ]
}
```

### 9.3 Subscription Flow

**Initial Connection**:
```http
GET /mytopic/json HTTP/1.1
Host: ntfy.sh

HTTP/1.1 200 OK
Content-Type: application/x-ndjson

{"id":"open001","time":1708012340,"event":"open","topic":"mytopic"}
```

**Keepalive (periodic)**:
```json
{"id":"keep001","time":1708012400,"event":"keepalive","topic":"mytopic"}
```

**Actual Message**:
```json
{"id":"msg001","time":1708012450,"event":"message","topic":"mytopic","message":"Hello"}
```

---

## 10. Implementation Checklist

### Phase 1: Minimum Viable Compatibility
- [ ] Parse topic from URL path
- [ ] Parse publish headers (Title, Priority, Tags, Message)
- [ ] Generate 12-char message ID
- [ ] Store message with timestamp
- [ ] Implement `POST /<topic>` (publish)
- [ ] Implement `GET /<topic>/json` (subscribe NDJSON stream)
- [ ] Send `open` event on new subscription
- [ ] Send `keepalive` events periodically
- [ ] Broadcast `message` events to subscribers
- [ ] Handle connection close/cleanup

### Phase 2: Enhanced Features
- [ ] Implement `since` query parameter (time & ID)
- [ ] Implement message expiration/TTL
- [ ] Add actions support
- [ ] Add attachment support
- [ ] Implement `sequence_id` for updates
- [ ] Add query filters (priority, tags)
- [ ] Multi-topic subscriptions
- [ ] WebSocket endpoint

### Phase 3: Production Ready
- [ ] Rate limiting
- [ ] Authentication/authorization
- [ ] Firebase integration (optional)
- [ ] Health/stats endpoints
- [ ] CORS configuration
- [ ] Comprehensive logging
- [ ] Metrics/monitoring
- [ ] Android app integration testing

---

## Document Version

- **Created**: 2026-02-14
- **Source Commit**: 7adb37b9 (main branch)
- **Last Updated**: 2026-02-14

**Maintenance Note**: When ntfy updates, check these files for breaking changes:
- `server/types.go` (message schema)
- `server/server.go` (endpoints, parameters)
- Android app expectations (search for "See Android" comments in code)

---

## Quick Reference Card

### Publish Message
```bash
curl -H "Title: Alert" \
     -H "Priority: urgent" \
     -H "Tags: warning" \
     -d "Message body" \
     https://ntfy.sh/mytopic
```

### Subscribe JSON
```bash
curl -s https://ntfy.sh/mytopic/json
```

### Subscribe Since Time
```bash
curl -s https://ntfy.sh/mytopic/json?since=1708012345
```

### Subscribe Multiple Topics
```bash
curl -s https://ntfy.sh/topic1,topic2,topic3/json
```

---

**End of Document**
