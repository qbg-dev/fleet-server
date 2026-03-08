# PROGRESS

## Current Phase: 5 (Attachments + Push)

## Endpoint Summary (20 endpoints operational)

| Method | Path | Description |
|--------|------|-------------|
| GET | /health | Health check |
| POST | /api/accounts | Register account |
| GET | /api/accounts/:id | Get profile ("me" supported) |
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
| POST | /api/lists | Create mailing list |
| POST | /api/lists/:id/subscribe | Subscribe |
| POST | /api/lists/:id/unsubscribe | Unsubscribe |
| POST | /api/webhooks/git-commit | Commit notification webhook |

## Test Summary: 62 tests, all passing
- 24 unit tests (storage + search + parser + filter)
- 14 integration tests (HTTP API)
- Duplicated across lib and bin crate targets

## Cycle Log

### Cycle 0 — Bootstrap (2026-03-08)
- Project scaffolded with Cargo.toml, MISSION.md, ROADMAP.md
- Rust toolchain installed on kevinster (1.94.0)
- Reference repos cloned for study
- tmux session `bms` created
- Chief-of-staff agent launched

### Cycle 1 — Phase 1+2 Complete (2026-03-08)
- Full storage layer: SqliteDataStore with 12 unit tests
- HTTP API: auth middleware, accounts, messages, labels, threads
- _diagnostics middleware on all responses
- 9 integration tests

### Cycle 2 — Phase 3+4 Complete (2026-03-08)
- FTS5 search with Gmail query parser (from:, to:, label:, has:attachment, before:, after:)
- CompiledQuery: parsed AST → SQL WHERE + FTS5 MATCH
- Batch modify endpoint
- Custom label CRUD
- Mailing lists (create, subscribe, unsubscribe)
- Git commit webhook
- Recycle readiness endpoint
- Total: 20 endpoints, 62 tests

## ROADMAP Status

### Phase 1: Foundation — COMPLETE ✓
### Phase 2: Core Messages — COMPLETE ✓
### Phase 3: Threading + Search — COMPLETE ✓
### Phase 4: Mailing Lists + Request/Response — COMPLETE ✓
- [x] Mailing lists: create, subscribe, unsubscribe
- [x] Request/response: reply_by field, pending replies in _diagnostics
- [x] Custom label CRUD + label list with unread counts
- [x] Batch modify
- [x] Webhook endpoint for git commit notifications
- [ ] Background: deadline expiry → auto-label OVERDUE (deferred to Phase 7)
- [ ] Fan-out at send time (list → individual copies) (deferred)

### Phase 5: Attachments + Push — IN PROGRESS
- [ ] Blob store (SHA-256, zstd, content-addressed)
- [ ] Upload/download blob endpoints
- [ ] Attach blobs to messages
- [ ] tmux push notification

### Phase 6: Registry Integration + Polish
- [x] Recycle readiness endpoint
- [ ] Auto-provision from registry.json
- [ ] Compression: zstd on message bodies
- [ ] mail-hook.sh script
- [ ] End-to-end conformance tests
