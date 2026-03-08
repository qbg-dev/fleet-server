# ROADMAP

## Phase 1: Foundation + HTTP Skeleton (get something running FAST)
- [ ] cargo init, dependencies (axum, tokio, tokio-rusqlite, rusqlite/bundled, serde, thiserror, sha2, zstd, uuid, chrono, tracing, tower-http)
- [ ] config.rs — bind addr, db path, blob dir
- [ ] schema.rs — all CREATE TABLE + FTS5 + indexes
- [ ] connection.rs — tokio-rusqlite, WAL + NORMAL sync + busy_timeout
- [ ] error.rs — StorageError, MessageError, ApiError with IntoResponse
- [ ] DataStore, BlobStore, SearchStore traits
- [ ] axum skeleton — router, bearer token auth middleware, health endpoint
- [ ] `boring-mail init` — create ~/.boring-mail/, setup DB, generate admin token, print config

## Phase 2: Core Messages + HTTP → DOGFOOD MILESTONE
- [ ] Account register + profile (POST /api/accounts, GET /api/accounts/{id})
- [ ] Message send (validate → create thread → assign labels → store → respond)
- [ ] Message list (GET /messages?label=INBOX&maxResults=20, pageToken pagination)
- [ ] Message get (GET /messages/{id}, auto-remove UNREAD)
- [ ] Message modify (POST /messages/{id}/modify, addLabelIds/removeLabelIds)
- [ ] Pre-created system labels (INBOX, SENT, TRASH, UNREAD, STARRED + issue/agent labels)
- [ ] `_diagnostics` middleware — unread count, pending replies, health in every response
- [ ] Cross-machine test: measure latency from Hetzner→kevinster, Mac→kevinster
- [ ] **START DOGFOODING**: orchestrator + workers switch from JSONL to mail HTTP API

## Phase 3: Threading + Search
- [ ] Thread resolution (explicit thread_id > in_reply_to parent > new thread)
- [ ] Thread list (GET /threads?label=INBOX, with snippet, count, participants)
- [ ] Thread get (all messages, chronological)
- [ ] FTS5 indexing (triggers on insert/delete, index uncompressed text)
- [ ] Gmail query parser (from:, to:, has:attachment, label:, date range, quoted phrases)
- [ ] Search endpoint (GET /search?q=...)

## Phase 4: Mailing Lists + Request/Response
- [ ] Mailing lists: create, subscribe, send → fan-out (dedup, cycle detection)
- [ ] Request/response: reply_by field, awaiting_reply filter, reply_requested flag
- [ ] Background: deadline expiry → auto-label OVERDUE
- [ ] Custom label CRUD + label list with unread counts
- [ ] Batch modify/delete
- [ ] Webhook endpoint for git commit notifications

## Phase 5: Attachments + Push
- [ ] Blob store (SHA-256, zstd >4KB, content-addressed filesystem)
- [ ] Upload blob (POST /blobs, multipart)
- [ ] Download blob (GET /blobs/{hash})
- [ ] Attach blobs to messages on send
- [ ] tmux push notification (batch, dead pane detection)

## Phase 6: Registry Integration + Polish
- [ ] Auto-provision accounts from worker-fleet registry.json
- [ ] Store mail bearer token in registry custom.mail_token field
- [ ] Compression: zstd on message bodies >512 bytes
- [ ] Recycle readiness endpoint (GET /api/accounts/{id}/pending)
- [ ] mail-hook.sh — git hook template for commit notifications
- [ ] End-to-end conformance tests (Gmail API shape verification)

## Phase 7+: Continuous Polish (Perpetual)
- [ ] Optional MCP stdio wrapper (thin shim calling HTTP)
- [ ] Refactoring: clean interfaces, reduce complexity
- [ ] Performance: profile hot paths, optimize queries
- [ ] Tests: edge cases, property-based tests, fuzzing
- [ ] CLI: `boring-mail status`, `boring-mail init`, `boring-mail accounts`
- [ ] Analytics: message volume, response times, per-agent stats
- [ ] Documentation: rustdoc, README, API reference
