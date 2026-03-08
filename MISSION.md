# boring_mail_server — Chief of Staff Mission

## Your Perpetual Loop

EVERY CYCLE:
1. Read PROGRESS.md for context from previous cycles
2. Search the web for latest multi-agent orchestration articles (Cursor Bugbot, OpenAI Codex agents, Devin, Claude agent patterns, perpetual harnesses). Save to docs/research/
3. Pick 1-3 tasks from the ROADMAP
4. Implement (directly or via sub-agents in bms:workers)
5. Spawn a verification agent to test (`cargo test` + edge cases)
6. Fix anything the verifier found
7. Commit with clear messages
8. Update PROGRESS.md
9. Sleep 10 min, repeat

## Architecture Reconsideration (Every 5 Cycles)

Every 5th cycle, STOP implementation and do a full architecture review:
1. Re-read all source files. Does the module structure still make sense?
2. Are there modules doing too much? Split them.
3. Are there abstractions that turned out wrong? Refactor now, not later.
4. Review error types — are they giving good diagnostics?
5. Check trait boundaries — are storage/service layers properly separated?
6. Write findings to docs/architecture-reviews/cycle-{N}.md
7. If a major refactor is needed, do it BEFORE resuming feature work.

## Agent Lifecycle Integration (CRITICAL — Learned from worker-fleet MCP)

The worker-fleet MCP server has battle-tested patterns for agent coordination. The mail server MUST integrate with these patterns at every stage of the agent lifecycle. Study `~/.claude-ops/mcp/worker-fleet/index.ts` (3,344 lines) for reference.

### Inbox Notifications via Hooks

Agents don't just poll — they get **pushed** notifications at key lifecycle events via git hooks and MCP tool call hooks:

1. **Post-commit hook** (`post-commit`): After every git commit, the hook appends a notification to relevant inboxes. The mail server should support a webhook endpoint (`POST /api/webhooks/git-commit`) that hooks can call to deliver commit notifications as mail messages to relevant agents.

2. **Prompt-publisher hook** (`prompt-publisher.sh`): When a user types into a worker's pane, a `worker-user-prompt` event is published to the coordinator's inbox. The mail server should support similar event-driven message injection — `POST /api/messages/send` with a `source: "hook"` field for traceability.

3. **MCP tool call hooks** (`PreToolUse`, `PostToolUse`): Claude's hook system fires before/after every tool call. Hooks can inject mail checks:
   - `PreToolUse` on `send_message` → auto-check for unread mail, append unread count to tool response
   - `PostToolUse` on `git commit` → notify relevant agents of the commit via mail

**Implementation**: Create a thin shell script (`scripts/mail-hook.sh`) that agents install as a git hook. On commit, it `curl`s the mail server to send a notification. Template:
```bash
#!/bin/bash
MAIL_SERVER="http://100.88.146.25:8025"
TOKEN="$BORING_MAIL_TOKEN"
COMMIT_SHA=$(git rev-parse HEAD)
COMMIT_MSG=$(git log -1 --pretty=%s)
AUTHOR=$(git log -1 --pretty=%an)
curl -sf -X POST "$MAIL_SERVER/api/messages/send" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{\"to\":[\"all\"],\"subject\":\"commit: $COMMIT_MSG\",\"body\":\"$AUTHOR committed $COMMIT_SHA\",\"labels\":[\"COMMIT\"]}" \
  >/dev/null 2>&1 &
```

### Linting & Diagnostics on Every API Call

The worker-fleet MCP appends diagnostics to ALL tool responses — pending replies, environment checks, unread counts. The mail server MUST do the same:

**Every API response includes a `_diagnostics` field:**
```json
{
  "data": { ... },
  "_diagnostics": {
    "unread_count": 3,
    "pending_replies": [
      {"msg_id": "abc123", "from": "bms-storage", "subject": "Need review", "reply_by": "2026-03-08T12:00:00Z"}
    ],
    "overdue_count": 1,
    "inbox_hint": "You have 3 unread messages. Use GET /api/messages?label=UNREAD to read them.",
    "health": {
      "db_size_mb": 12.4,
      "blob_count": 47,
      "uptime_secs": 3600
    }
  }
}
```

**Why this matters**: Agents are forgetful. They don't proactively check their inbox. By appending unread counts and pending reply reminders to EVERY response, agents are constantly reminded of messages awaiting their attention. This is the single most effective pattern from the worker-fleet MCP — it turns passive inboxes into active notifications.

**Implementation priority**: This is a Phase 2 deliverable. The `_diagnostics` middleware wraps all axum responses and queries the DB for the authenticated user's unread count + pending replies. Cache with 5-second TTL to avoid per-request DB hits.

### Agent Lifecycle Checkpoints

At each stage of an agent's lifecycle, the mail server should provide appropriate functionality:

| Lifecycle Stage | Mail Server Role | Worker-Fleet Pattern |
|----------------|-----------------|---------------------|
| **Registration** | `POST /api/accounts` creates mailbox. Auto-provision from registry.json. | `register()` → registry entry |
| **Task Assignment** | Coordinator sends task via mail with `reply_requested: true, reply_by: <deadline>` | `create_task()` + `send_message()` |
| **Working** | Agent sends progress updates. Hooks deliver commit notifications. | `update_state()` + post-commit hook |
| **Communication** | Direct messages, mailing lists, threaded discussions | `send_message()` + `read_inbox()` |
| **Blocked** | Agent sends message with `labels: ["BLOCKED"]`. Coordinator gets notified. | `update_state({status: "blocked"})` |
| **Verification** | Verifier agent sends review results as threaded reply | `send_message(reply_type: "review")` |
| **Recycle** | Agent sends handoff summary to own inbox before restart. New session reads it on startup. | `recycle()` → HANDOFF.md |
| **Deregistration** | Mailbox preserved (audit trail). Account marked inactive. | `deregister()` → HANDOFF.md required |

### Stop Check Integration

Before an agent recycles (restarts), it should verify all pending mail is handled:
- No `reply_requested` messages left unanswered
- All in-progress threads have a status update
- Handoff summary sent to own inbox for next session

The mail server exposes `GET /api/accounts/{id}/pending` which returns:
```json
{
  "unanswered_requests": [...],
  "in_progress_threads": [...],
  "ready_to_recycle": false,
  "blockers": ["2 unanswered reply requests"]
}
```

Agents call this before `recycle()` and address blockers first.

## Compression

All stored message bodies and large text fields should support optional compression:
- Use zstd (via `zstd` crate) — best ratio for small payloads, dictionary support, fast decompression
- Compress message bodies > 512 bytes before SQLite INSERT
- Store a `compressed: bool` column (or prefix byte) so reads decompress transparently
- Blob store: compress blobs > 4KB before writing to disk (content-address the compressed form)
- Thread snippets: store uncompressed (short, frequently read)
- FTS5: index the UNCOMPRESSED text (search must work on plaintext)
- Benchmark: log compression ratio in PROGRESS.md after Phase 2

## Perpetual Execution

You MUST run perpetually. If you finish all ROADMAP items, shift to:
1. Optimization passes (benchmark, profile, improve hot paths)
2. Additional test coverage (fuzzing, property-based tests)
3. Documentation (rustdoc, README, usage examples)
4. Research new patterns from industry (web search each cycle)
Never exit. If blocked, log the blocker in PROGRESS.md and move to the next unblocked task.

## Spawning Sub-Agents (via worker-fleet MCP)

Use the claude-ops worker-fleet MCP infrastructure, NOT raw tmux commands:

```
# Create a worker for an implementation task
create_worker(
  name: "bms-storage",
  model: "opus",
  mission: "Implement DataStore trait and SQLite backend for boring_mail_server. TDD. cargo test must pass.",
  branch: "bms/storage",
  perpetual: false,
  window: "bms-workers"
)

# Create a verification worker
create_worker(
  name: "bms-verifier",
  model: "opus",
  mission: "Run cargo test on boring_mail_server. Review code for correctness. Report findings.",
  branch: "main",
  perpetual: false,
  window: "bms-workers"
)

# Communicate with workers via send_message / read_inbox
send_message(to: "bms-storage", content: "TASK: Implement message send pipeline", summary: "task assignment")
read_inbox()  # Check for worker completions
```

Workers register in the fleet registry. Once the mail server is running, switch to using it for coordination (dogfooding).

## Recycle for Reload

When the mail server binary is updated, recycle the orchestrator:
```
recycle()  # Restarts Claude session, picks up new PROGRESS.md, reloads mail server
```

## Dogfooding Priority (CRITICAL)

Get the mail server to a USABLE state ASAP. The build order is:
1. **Phase 1-2**: Foundation + basic send/list/get → **START DOGFOODING HERE**
2. Orchestrator + workers switch from JSONL inbox to mail HTTP API
3. Continue building remaining features (threading, search, labels) while using the mail server for coordination
4. Every bug found via dogfooding gets fixed immediately

The mail server's first real users are its own builders. This is the fastest path to correctness.

## Reference Implementations (STUDY THESE)

- ~/references/stalwart — Rust. Study storage layer, JMAP threading, SQLite integration
- ~/references/mox — Go. Study simplicity, self-contained design, admin API
- ~/references/mcp_agent_mail — Python. Study agent-specific features (file leases, FTS5, macros)
- ~/references/james — Java. Study modular architecture, protocol separation

### Reference Learnings Summary

| Reference | Steal | Avoid |
|-----------|-------|-------|
| **Stalwart** (Rust) | 4-store trait separation (DataStore/BlobStore/SearchStore), JMAP threading algorithm | 8 backend options—we need one |
| **mox** (Go) | Zero-dep binary, `localserve` mode, one-command quickstart (`boring-mail init`) | SMTP/IMAP/DKIM complexity |
| **mcp_agent_mail** (Python→Rust) | Failure mode lessons (git locks, pool exhaustion under multi-agent), commit coalescer pattern | Git as primary storage, MCP-only transport |
| **James** (Java) | Protocol/storage/processing separation, mailet-style processing pipeline | Java enterprise patterns, Cassandra/RabbitMQ |

### Linear Philosophy (Steal These)

1. **Speed is a feature.** Every operation <10ms. SQLite WAL + Rust makes this achievable. If `list_messages` takes >10ms, investigate.
2. **Opinionated defaults.** Pre-created system labels with auto-assignment. No empty-canvas setup. `boring-mail init` → ready to use.
3. **Triage as forcing function.** Messages with `reply_requested` that aren't acted on → auto-label `NEEDS_TRIAGE` after deadline. tmux push is the triage prompt. Prevents passive inbox rot.
4. **Cycles.** Future extension: auto-generate daily digest threads summarizing activity. Not v1.

## What to Build

A Gmail-conformant mail server for AI agents. HTTP REST API (axum) + optional MCP stdio wrapper.

### Gmail API Conformance

**Follow exactly:** URL structure (`/api/...`), resource shapes (Message, Thread, Label), `me` shorthand, `{addLabelIds, removeLabelIds}` modify, `pageToken`/`nextPageToken` pagination, `?q=` search, `format` param (full/metadata/minimal), system labels (INBOX, SENT, TRASH, UNREAD, STARRED, DRAFT).

**Deviate (simplify):**
- Message body: plain JSON `{to, cc, subject, body}` (not RFC 2822 MIME base64url)
- Auth: Bearer token (not OAuth2)
- No Drafts, Settings, or History resources
- Attachments: content-addressed blob store with `POST /api/blobs` (not inline base64 in payload.parts[])
- Batch: dedicated `batchModify`/`batchDelete` endpoints (not multipart MIME `/batch`)

**Three extensions beyond Gmail:**
1. **Mailing lists** — `POST /api/lists` to create, `POST /api/lists/{id}/subscribe`. Send to a list address → all subscribers get the message in their inbox.
2. **Request/response semantics** — optional `reply_by` field on send (ISO timestamp). Server tracks pending responses. `GET /api/messages?awaiting_reply=true` returns messages you sent that haven't been replied to yet. Recipient sees a `reply_requested` flag.
3. **tmux push notifications** — on message delivery, paste a one-liner into the recipient's registered tmux pane: `[MAIL] From: {from} | Subject: {subject} | Thread: {thread_id}`. Agents register their pane via `POST /api/accounts/{id}/pane` with `{pane_id}`.

### Threading Algorithm
- Optional `thread_id` on send. If provided and exists, message joins that thread.
- If `in_reply_to` message_id provided, inherit parent message's thread_id.
- Otherwise, create a new thread.
- Thread table is materialized (not a view) — updated within the message-insert transaction.

### Data Model (SQLite + rusqlite)
- **accounts** — id, name, display_name, bearer_token, tmux_pane_id, active, created_at
- **messages** — id, thread_id, from_account, subject, body, snippet, has_attachments, internal_date, in_reply_to, reply_by, reply_requested, compressed, history_id, source
- **message_recipients** — message_id, account_id, recipient_type (to/cc)
- **threads** — id, subject, snippet, last_message_at, message_count, participants (materialized, synced on insert)
- **message_labels** — message_id, account_id, label (per-account label assignment)
- **labels** — id, account_id, name, type (system/user), message_count, unread_count
- **attachments** — message_id, blob_hash, filename, content_type, size
- **blobs** — hash (PK), size, compressed, created_at (actual files at ~/.boring-mail/blobs/{hash}.zst)
- **messages_fts** — FTS5 virtual table on subject + body (indexed from uncompressed text)
- **lists** — id, name, description, created_at
- **list_members** — list_id, account_id
- **audit_log** — id, actor, action, resource_type, resource_id, details, created_at

### HTTP REST API (20 endpoints, Gmail-shaped)

Transport: axum on `0.0.0.0:8025`. Auth: Bearer token per account.

**Messages (7):**
- `POST /api/messages/send` — send message
- `GET /api/messages?label=INBOX&maxResults=20` — list by label, paginated
- `GET /api/messages/{id}` — get (auto-removes UNREAD)
- `POST /api/messages/{id}/modify` — add/remove labels
- `POST /api/messages/batchModify` — batch label changes
- `POST /api/messages/{id}/trash` — move to trash
- `DELETE /api/messages/{id}` — permanent delete

**Threads (3):**
- `GET /api/threads?label=INBOX&maxResults=20` — list threads
- `GET /api/threads/{id}` — get thread (all messages)
- `POST /api/threads/{id}/trash` — trash thread

**Attachments (2):**
- `POST /api/blobs` — upload (multipart)
- `GET /api/blobs/{hash}` — download

**Labels (3):**
- `GET /api/labels` — list (with unread counts)
- `POST /api/labels` — create custom label
- `DELETE /api/labels/{id}` — delete

**Account (3):**
- `POST /api/accounts` — register
- `GET /api/accounts/{id}` — get profile
- `GET /api/accounts/{id}/pending` — recycle readiness check

**Search (1):**
- `GET /api/search?q=merge+request&from=merger` — FTS5 search

**Webhooks (1):**
- `POST /api/webhooks/git-commit` — hook-driven commit notifications

### Pre-Created Labels (Linear-style opinionated defaults)
System labels (immutable, auto-assigned):
- `INBOX`, `SENT`, `TRASH`, `UNREAD`, `STARRED`, `DRAFT`

Issue workflow labels (pre-created, optional):
- `ISSUE`, `OPEN`, `IN_PROGRESS`, `RESOLVED`, `WONTFIX`

Priority labels:
- `P0`, `P1`, `P2`, `P3`

Agent lifecycle labels:
- `COMMIT`, `BLOCKED`, `OVERDUE`, `NEEDS_TRIAGE`

Auto-assignment on send: recipient gets `INBOX + UNREAD`, sender gets `SENT`. Messages with `ISSUE` label auto-get `OPEN`.

### Agent-Specific Features
- **Push via tmux**: On send, paste-buffer one-liner to recipient's registered pane. Batch: collapse N messages in 1 second into "you have N new messages". Dead pane detection before send.
- **Request/response**: Optional `reply_by` ISO timestamp. Server tracks pending. `?awaiting_reply=true` filter. Auto-label `OVERDUE` when deadline passes (background task).
- **Mailing lists**: `POST /api/lists`, subscribe/unsubscribe. Fan-out at send time (each recipient gets own copy). Dedup: if direct recipient is also list member, one copy. Cycle detection for nested lists.
- **Diagnostics middleware**: Every response includes `_diagnostics` with unread count, pending replies, overdue items.

### Explicit Non-Features
- **No file leases.** Agents work in their own git worktrees—file conflicts are solved by isolation, not locks.
- **No git-backed storage.** Audit log table is sufficient. Git adds lock contention under multi-agent load.
- **No SMTP/IMAP.** HTTP REST only. No email protocol complexity.
- **No web UI.** Agents use the API. Warren uses curl or a future Linear sync.

## Priorities (NON-NEGOTIABLE)

1. **INTERFACES FIRST** — Design the trait signatures, data structures, and API contracts BEFORE writing any implementation. The interfaces ARE the architecture. Review them. Make them feel good to use. Iterate on them. A beautiful interface with a mediocre implementation beats the reverse every time.
2. **TEST-DRIVEN DEVELOPMENT** — Write the test FIRST. Watch it fail. Implement. Watch it pass. `cargo test` must pass after EVERY commit. No exceptions.
3. **CORRECTNESS** — Every message delivered exactly once. Every thread correctly grouped. Every label transition valid. Type system enforces invariants where possible.
4. **CLEAN BOUNDARIES** — Traits for storage backends. Each module has one job. Good error types (thiserror, per-module). No god objects. Simple > clever. Functions < 50 lines.
5. **PERFORMANCE** — Every operation <10ms (Linear's bar). WAL mode, prepared statements, zero-copy where sensible.

## Key Architecture Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Storage traits | 3 traits: `DataStore`, `BlobStore`, `SearchStore` | Split by access pattern, not per-resource |
| Async bridge | `tokio-rusqlite` (dedicated thread + channel) | Single thread owns SQLite. Clone+Send handle in axum State |
| Threads | Materialized table, updated in message-insert transaction | Thread listing is hot path |
| Mailing lists | Fan-out at send time | Each recipient gets own copy with own labels |
| Blob storage | Store everything forever | Storage is cheap, auditability is priceless |
| Error types | Per-module with thiserror → `ApiError` → `IntoResponse` | Impossible states unrepresentable |
| Auth | One bearer token per registered account. Mailbox isolation enforced | Can't read other agents' mail |
| Diagnostics | `_diagnostics` appended to ALL responses (5s TTL cache) | Agents are forgetful — constant reminders |

## Project Structure

```
boring_mail_server/
├── Cargo.toml
├── MISSION.md
├── PROGRESS.md
├── ROADMAP.md
├── docs/research/
├── scripts/
│   ├── disk-monitor.sh
│   └── mail-hook.sh           # Git hook template for commit notifications
├── src/
│   ├── main.rs                # Entry point, CLI args, router assembly
│   ├── lib.rs
│   ├── config.rs              # Server config (bind addr, paths, tokens)
│   ├── db/
│   │   ├── schema.rs          # CREATE TABLE + FTS5 + indexes
│   │   ├── migrations.rs
│   │   └── connection.rs      # tokio-rusqlite setup, WAL config
│   ├── storage/
│   │   ├── mod.rs             # DataStore, BlobStore, SearchStore traits
│   │   ├── sqlite.rs          # DataStore impl
│   │   ├── blob.rs            # BlobStore impl (SHA-256, zstd, filesystem)
│   │   ├── fts.rs             # SearchStore impl (FTS5)
│   │   └── models.rs          # Internal row types
│   ├── service/
│   │   ├── messages.rs        # send pipeline, list, get, modify, trash, delete
│   │   ├── threads.rs         # list, get, modify, trash
│   │   ├── labels.rs          # CRUD + system label management
│   │   ├── lists.rs           # Mailing list expansion
│   │   ├── accounts.rs        # Registration, profile, pane registration
│   │   └── requests.rs        # Request/response tracking (reply_by)
│   ├── search/
│   │   ├── parser.rs          # Gmail query syntax → AST
│   │   ├── fts.rs             # AST → FTS5 MATCH
│   │   └── filter.rs          # AST → SQL WHERE
│   ├── api/
│   │   ├── mod.rs             # axum Router construction
│   │   ├── messages.rs        # HTTP handlers
│   │   ├── threads.rs
│   │   ├── labels.rs
│   │   ├── accounts.rs
│   │   ├── search.rs
│   │   ├── webhooks.rs        # Hook-driven endpoints
│   │   ├── auth.rs            # Bearer token extraction middleware
│   │   ├── diagnostics.rs     # _diagnostics middleware (unread, pending, health)
│   │   ├── error.rs           # ApiError → IntoResponse
│   │   └── models.rs          # API request/response types
│   ├── background/
│   │   ├── mod.rs             # tokio::spawn interval tasks
│   │   └── deadlines.rs       # reply_by expiry → OVERDUE labeling
│   ├── delivery/
│   │   └── tmux.rs            # Push notification via paste-buffer
│   ├── mcp/
│   │   ├── server.rs          # Optional MCP stdio wrapper
│   │   └── tools.rs           # Tool definitions calling service/
│   └── error.rs               # Per-module error types
└── tests/
    ├── common/mod.rs           # Test helpers, in-memory SQLite setup
    ├── messages.rs
    ├── threads.rs
    ├── search.rs
    ├── lists.rs
    ├── attachments.rs
    └── integration.rs          # Full HTTP API flow tests
```

## Network Topology

```
Warren's Mac ──(Tailscale)──→ kevinster:8025 (boring-mail HTTP)
Hetzner VPS  ──(Tailscale)──→ kevinster:8025
Kevinster    ──(localhost)──→ kevinster:8025
```

Server binds `0.0.0.0:8025` with bearer token auth. Performance target: <50ms round-trip cross-machine.

## Cross-Machine Latency Test (After Phase 2)

```bash
# From Hetzner (Ashburn → kevinster via Tailscale)
time curl -s -H "Authorization: Bearer $TOKEN" http://100.88.146.25:8025/api/messages?label=INBOX&maxResults=10

# From Warren's Mac
time curl -s -H "Authorization: Bearer $TOKEN" http://100.88.146.25:8025/api/messages?label=INBOX&maxResults=10
```

Target: <50ms from Warren's Mac, <100ms from Hetzner.
