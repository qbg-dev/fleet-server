# PROGRESS

## Current Phase: 7 (Continuous Polish)

## Endpoint Summary (24 endpoints operational)

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
| GET | /api/analytics | System-wide + per-account stats |
| POST | /api/webhooks/git-commit | Commit notification webhook |

## Test Summary: 196 tests, all passing
- 55 unit tests (storage, search, parser, filter, blob, tmux, analytics, 16 property-based, 4 pagination)
- 73 integration tests (HTTP API + conformance + edge cases + analytics + hardening + middleware + pagination + request-id + coverage + security + rate-limit)
- 8 MCP protocol tests (initialize, tools/list, ping, error handling)
- 4 CLI tests (help, init, status, accounts)
- 1 performance benchmark (send, list, get, search, labels)
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
- Performance benchmark: all hot paths under 10ms target
  - send_message: ~500µs, list_messages: ~940µs, get_message: ~430µs
  - search: ~1.1ms, list_labels: ~590µs
- 103 tests (33 unit + 28 integration + 8 MCP + 1 bench)

### Cycle 7 — CLI + refactoring (2026-03-08)
- clap-based CLI: `serve` (default), `init`, `status`, `accounts`
- `init`: creates data dir + DB, prints paths
- `status`: shows DB stats (accounts/messages/threads), blob dir, server health check
- `accounts`: lists all registered accounts in table format
- Added `list_accounts` to DataStore trait
- Refactored `insert_message` (187→70 lines): extracted resolve_thread, compress_body, insert_recipients_and_labels, update_thread_metadata helpers
- Resolved all clippy warnings
- 4 CLI integration tests (help, init, status, accounts)
- 109 tests (34 unit + 28 integration + 8 MCP + 4 CLI + 1 bench)

### Cycle 8 — Analytics endpoint (2026-03-08)
- GET /api/analytics: system-wide totals + per-account stats (sent, received, threads, unread)
- Analytics model + DataStore trait method + SQLite queries
- 1 unit test + 1 integration test
- 112 tests (35 unit + 29 integration + 8 MCP + 4 CLI + 1 bench)
- 24 endpoints total

### Cycle 9 — Edge case tests + input validation (2026-03-08)
- Label name validation: reject empty, whitespace-only, and >256 char names (400 error)
- 12 new edge case integration tests:
  - System label deletion protection (INBOX, SENT, TRASH, UNREAD, STARRED)
  - Empty/whitespace label name creation rejected
  - Long label name rejected
  - Nonexistent message GET/modify/trash/delete → 404
  - Empty recipients → 400
  - Nonexistent mailing list recipient → 400
  - Duplicate account name → error
  - Batch modify with empty IDs → 400
  - Empty inbox pagination (null nextPageToken)
- 124 tests (35 unit + 41 integration + 8 MCP + 4 CLI + 1 bench)

### Cycle 10 — Documentation (2026-03-08)
- README.md: quick start, API reference (24 endpoints), architecture diagram, CLI, MCP, config
- Rustdoc on core public types: lib.rs module docs, Config, error enums, storage traits, all 13 data models
- Zero clippy warnings, zero rustdoc warnings
- 124 tests still passing

### Cycle 11 — Property-based tests + Unicode bug fix (2026-03-08)
- Added proptest dependency for property-based testing
- 16 new property-based tests across 3 modules:
  - Query parser (6): parse never panics, whitespace→empty, operator extraction, label uppercasing, fts_query consistency
  - Body compression (3): compress→decompress roundtrip, short bodies uncompressed, long bodies compressed
  - Snippet (3): char count bounded, short body preservation, long body ellipsis
  - Blob store (4): store→get roundtrip, deterministic hash, different data→different hash, meta size matches
- **Bug found by proptest**: `make_snippet` used `body.len()` (byte count) instead of `body.chars().count()` for truncation check—multi-byte Unicode chars caused incorrect truncation/preservation behavior
- 140 tests (51 unit + 41 integration + 8 MCP + 4 CLI + 1 bench)
- Zero clippy warnings

### Cycle 12 — Production hardening (2026-03-08)
- Request body size limit: `DefaultBodyLimit` layer (10MB default, configurable via `BORING_MAIL_MAX_BODY`)
- Request timeout: `TimeoutLayer` with 408 status code (30s default, configurable via `BORING_MAIL_TIMEOUT`)
- Graceful shutdown: SIGTERM/SIGINT handler drains in-flight requests before exit
- 2 new integration tests: oversized body rejected (413), small body accepted
- 142 tests (51 unit + 43 integration + 8 MCP + 4 CLI + 1 bench)
- Zero clippy warnings

### Cycle 13 — Tower-http middleware activation (2026-03-08)
- Wired up 3 tower-http layers that were declared in Cargo.toml but unused:
  - **CorsLayer**: permissive CORS (allow any origin/method/header) for cross-origin agent access
  - **CompressionLayer**: gzip response compression for large payloads
  - **TraceLayer**: structured request/response logging (method, path, status, latency)
- 4 new integration tests: CORS headers present, CORS allows any origin, gzip encoding, health version
- 146 tests (51 unit + 47 integration + 8 MCP + 4 CLI + 1 bench)
- Zero clippy warnings

### Cycle 14 — Pagination fix + refactoring (2026-03-08)
- **Bug fix**: pagination used `<` comparison with extra item's timestamp as token,
  causing the boundary item to be dropped on next page. Fixed to use `<=`.
- Extracted 3 new helpers from sqlite.rs:
  - `paginate_results`: shared pagination logic for list_messages/list_threads (eliminates .pop().unwrap())
  - `attach_blobs`: extracted from insert_message
  - `build_sent_message`: extracted from insert_message (now 48 lines, was 80)
- 4 new unit tests: paginate_results (empty, under, at, over limit)
- 4 new integration tests: exactly-at-limit, one-over-limit, single message, thread pagination boundary
- 154 tests (55 unit + 51 integration + 8 MCP + 4 CLI + 1 bench)
- Zero clippy warnings

### Cycle 15 — Refactoring + request ID (2026-03-08)
- Extracted `row_to_message` and `row_to_thread` helpers: eliminated 3x duplicated row-mapping code
- Extracted `run_paginated_list` with `PaginatedQuery` struct: shared query-building + pagination for list_messages/list_threads
- Extracted `query_per_account_stats` from get_analytics, consolidated 4 COUNT queries into 1 SELECT
- get_analytics: 58→28 lines, list_messages: 73→40 lines, list_threads: 67→38 lines
- **New feature**: x-request-id header (UUID v4) on all responses via tower-http SetRequestIdLayer + PropagateRequestIdLayer
- 2 new integration tests: request-id presence + uniqueness
- 156 tests (55 unit + 53 integration + 8 MCP + 4 CLI + 1 bench)
- Zero clippy warnings

### Cycle 16 — Coverage tests + refactoring (2026-03-08)
- 9 new integration tests covering previously untested scenarios:
  - CC recipients, UNREAD auto-removal on read, thread reply ordering
  - Nonexistent thread 404, search empty/multi-word results
  - Delete nonexistent label, simultaneous add+remove modify, blob dedup
- 165 tests (55 unit + 62 integration + 8 MCP + 4 CLI + 1 bench)
- Zero clippy warnings

### Cycle 17 — Rate limiting + security hardening (2026-03-08)
- **Rate limiting**: per-account sliding window (DashMap), configurable via `BORING_MAIL_RATE_LIMIT` (req/min, default 60, 0=unlimited)
- **Security fixes** (found by dogfooding agent fleet):
  - get_message: 403 for non-participants (IDOR)
  - delete_message: 403 for non-participants (IDOR)
  - get_account: block IDOR (only own account)
  - git-commit webhook: require bearer auth
  - search handler: propagate errors instead of silently dropping
- **Bug fixes**: duplicate label → 409, duplicate account → 409
- **Input validation**: create_account, create_list, create_label (empty, whitespace, length, trimming)
- Fixed flaky test_cli_status (was detecting running dogfood server)
- Resolved clippy warning in rate_limit middleware
- 8 new security integration tests + 3 rate limit tests
- 196 tests total (55 unit + 73 integration + 8 MCP + 4 CLI + 1 bench)
- Zero clippy warnings
- New dependency: dashmap 6

### Phase 7+: Continuous Polish
- [x] MCP stdio wrapper
- [x] Performance profiling (all ops <10ms, see Cycle 6 benchmarks)
- [x] Background OVERDUE labeling (already implemented in Cycle 2-3)
- [x] Mailing list fan-out on send
- [x] CLI subcommands (serve, init, status, accounts)
- [x] Refactoring: insert_message decomposition (187→48 lines)
- [x] Analytics endpoint (per-account + system-wide stats)
- [x] Documentation: README, rustdoc on core public types
- [x] Property-based tests (proptest: parser, compression, blob, snippet)
- [x] Production hardening: body size limit, request timeout, graceful shutdown
- [x] Tower-http middleware: CORS, gzip compression, request tracing
- [x] Pagination boundary fix + tests
- [x] Refactoring: row_to_message/row_to_thread/run_paginated_list/query_per_account_stats helpers
- [x] Request ID: x-request-id UUID v4 header on all responses
- [x] Coverage tests: CC, UNREAD removal, thread ordering, search, labels, blob dedup
- [x] Rate limiting: per-account sliding window, DashMap, configurable BORING_MAIL_RATE_LIMIT
- [x] Security hardening: ownership checks on get/delete message, account IDOR fix, webhook auth
- [x] Input validation: account/list/label name validation (empty, whitespace, length, trimming)
- [x] Error hygiene: duplicate label/account → 409 Conflict, search error propagation
