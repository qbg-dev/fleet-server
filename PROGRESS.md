# PROGRESS

## Current Phase: 2 (Core Messages — DOGFOOD MILESTONE)

## Cycle Log

### Cycle 0 — Bootstrap (2026-03-08)
- Project scaffolded with Cargo.toml, MISSION.md, ROADMAP.md
- Rust toolchain installed on kevinster (1.94.0)
- Reference repos cloned for study
- tmux session `bms` created
- Chief-of-staff agent launched

### Cycle 1 — Foundation + HTTP API (2026-03-08)

**Phase 1 — COMPLETE:**
- [x] Cargo.toml with all dependencies (axum, tokio, tokio-rusqlite, serde, thiserror, etc.)
- [x] config.rs — bind addr, db path, blob dir, admin token
- [x] schema.rs — all CREATE TABLE + FTS5 + indexes + system label seeding
- [x] connection.rs — tokio-rusqlite, WAL mode, NORMAL sync, busy_timeout
- [x] error.rs — StorageError, MessageError, ApiError with IntoResponse
- [x] DataStore, BlobStore, SearchStore traits defined
- [x] SqliteDataStore — full implementation (accounts, messages, labels, threads, diagnostics)
- [x] 12 unit tests for storage layer (all passing)
- [x] axum skeleton with health endpoint

**Phase 2 — COMPLETE:**
- [x] Account register (POST /api/accounts) + get profile (GET /api/accounts/:id)
- [x] Bearer token auth middleware (extracts Account from Authorization header)
- [x] Message send (POST /api/messages/send) — validates, creates thread, assigns labels
- [x] Message list (GET /api/messages?label=INBOX&maxResults=20) — cursor pagination
- [x] Message get (GET /api/messages/:id) — auto-removes UNREAD
- [x] Message modify (POST /api/messages/:id/modify) — addLabelIds/removeLabelIds
- [x] Message trash (POST /api/messages/:id/trash) — moves INBOX→TRASH
- [x] Message delete (DELETE /api/messages/:id) — permanent delete
- [x] Labels list (GET /api/labels) — with unread counts
- [x] Thread list (GET /api/threads?label=INBOX) — cursor pagination
- [x] Thread get (GET /api/threads/:id) — all messages chronological
- [x] `_diagnostics` middleware — unread_count, pending_replies, overdue_count, inbox_hint
- [x] Pre-created system labels (INBOX, SENT, TRASH, UNREAD, STARRED, DRAFT + issue/agent labels)
- [x] 9 integration tests (HTTP flow, auth, labels, threads, diagnostics)

**Test Summary: 33 tests total, all passing**
- 12 unit tests (storage layer)
- 9 integration tests (HTTP API)
- Duplicated in both lib and bin crate targets

**Next: Phase 3 — Threading + Search**
- Thread resolution already works (explicit thread_id > in_reply_to > new)
- FTS5 table exists, indexing happens on insert
- Need: Gmail query parser, search endpoint, thread list/get refinements
