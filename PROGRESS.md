# PROGRESS

## Current Phase: 7 (Continuous Polish)

## Endpoint Summary (23 endpoints operational)

| Method | Path | Description |
|--------|------|-------------|
| GET | /health | Health check |
| POST | /api/accounts | Register account |
| GET | /api/accounts/:id | Get profile ("me" supported) |
| POST | /api/accounts/:id/pane | Register tmux pane for notifications |
| GET | /api/accounts/:id/pending | Recycle readiness check |
| POST | /api/messages/send | Send message |
| GET | /api/messages | List by label, paginated |
| GET | /api/messages/:id | Get (auto-removes UNREAD) |
| POST | /api/messages/:id/modify | Add/remove labels |
| POST | /api/messages/:id/trash | Move to TRASH |
| DELETE | /api/messages/:id | Permanent delete |
| POST | /api/messages/batchModify | Bulk label changes |
| GET | /api/labels | List with counts |
| POST | /api/labels | Create custom label |
| DELETE | /api/labels/:name | Delete custom label |
| GET | /api/threads | List threads by label |
| GET | /api/threads/:id | Get thread with messages |
| GET | /api/search?q= | FTS5 + Gmail query syntax |
| POST | /api/blobs | Upload blob |
| GET | /api/blobs/:hash | Download blob |
| POST | /api/lists | Create mailing list |
| POST | /api/lists/:id/subscribe | Subscribe |
| POST | /api/lists/:id/unsubscribe | Unsubscribe |
| POST | /api/webhooks/git-commit | Commit notification webhook |

## Test Summary: 102 tests, all passing
- 33 unit tests (storage, search, parser, filter, blob, tmux)
- 28 integration tests (HTTP API + conformance + edge cases)
- 8 MCP protocol tests (initialize, tools/list, ping, error handling)
- Duplicated across lib and bin crate targets

## Release Binary: 5.1MB (stripped, thin LTO)

## Cycle Log

### Cycle 0 — Bootstrap (2026-03-08)
- Project scaffolded
- Rust toolchain installed on kevinster (1.94.0)

### Cycle 1 — Phase 1+2 (2026-03-08)
- Full storage layer with 12 unit tests
- HTTP API: auth, accounts, messages, labels, threads
- _diagnostics middleware
- 9 integration tests

### Cycle 2 — Phase 3-5 (2026-03-08)
- FTS5 search with Gmail query parser
- Batch modify, custom label CRUD
- Mailing lists, git commit webhook
- Recycle readiness endpoint
- Content-addressed blob store with zstd compression
- mail-hook.sh updated to use webhook endpoint
- Total: 22 endpoints, 76 tests

### Cycle 3 — Phase 5-6 completion (2026-03-08)
- Attach blobs to messages on send (attachments field + GET response)
- zstd compression on message bodies > 512 bytes (base64-encoded in DB)
- Auto-provision accounts from worker-fleet registry.json
- 76 tests (30 unit + 16 integration)

### Cycle 4 — Phase 5-6 finish + conformance (2026-03-08)
- tmux push notifications: pane registration, alive detection, fire-and-forget on send
- POST /api/accounts/:id/pane endpoint (23 endpoints total)
- Microsecond-precision timestamps (fix pagination with rapid sends)
- 4 new conformance tests: pagination, label CRUD, modify shapes, search shapes
- 86 tests (32 unit + 22 integration)

## ROADMAP Status

### Phase 1: Foundation — COMPLETE ✓
### Phase 2: Core Messages — COMPLETE ✓
### Phase 3: Threading + Search — COMPLETE ✓
### Phase 4: Mailing Lists + Request/Response — COMPLETE ✓
### Phase 5: Attachments + Push — COMPLETE ✓
- [x] Blob store (SHA-256, zstd >4KB, content-addressed, dedup)
- [x] Upload blob (POST /api/blobs)
- [x] Download blob (GET /api/blobs/:hash)
- [x] Attach blobs to messages on send
- [x] tmux push notification (pane registration, dead pane detection, fire-and-forget)

### Phase 6: Registry Integration + Polish — COMPLETE ✓
- [x] Recycle readiness endpoint
- [x] mail-hook.sh script
- [x] Auto-provision from registry.json
- [x] Compression: zstd on message bodies >512 bytes
- [x] End-to-end conformance tests (pagination, label CRUD, modify, search shapes)

### Cycle 5 — Mailing list fan-out + polish (2026-03-08)
- Mailing list fan-out on send: `list:name` prefix in recipients auto-expands to subscribers
- Subscribe endpoint accepts optional `account_id` body for admin subscriptions
- Proper 400 error for nonexistent mailing list recipients (was 500 FK error)
- Resolved all clippy warnings
- 5 new edge case tests: thread reply chains, diagnostics unread tracking, body compression roundtrip, empty/nonexistent recipient handling
- 94 tests (33 unit + 28 integration)

### Cycle 6 — MCP stdio wrapper (2026-03-08)
- `boring-mail-mcp` binary: MCP JSON-RPC 2.0 server over stdin/stdout
- 9 tools: send_message, read_inbox, get_message, search_messages, modify_labels, trash_message, list_labels, list_threads, get_thread
- Thin HTTP proxy via reqwest — configurable via BORING_MAIL_URL + BORING_MAIL_TOKEN env vars
- Full MCP protocol: initialize, notifications/initialized, tools/list, tools/call, ping
- Proper error handling: parse errors, method not found, unknown tools, missing args
- 8 new tests: protocol, tool listing, notifications, error cases
- 102 tests (33 unit + 28 integration + 8 MCP)

### Phase 7+: Continuous Polish
- [x] MCP stdio wrapper
- [ ] Performance profiling
- [ ] Background OVERDUE labeling
- [x] Mailing list fan-out on send
