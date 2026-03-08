# boring_mail_server

A Gmail-conformant mail server for AI agents. Rust + axum + SQLite.

## Commands

```bash
source ~/.cargo/env

# Build
cargo build                    # Debug build
cargo build --release          # Release build
cargo check                    # Type check only (fast)

# Test
cargo test                     # Run all tests
cargo test messages            # Run message tests
cargo test -- --nocapture      # Show println output

# Run
cargo run                      # Start server on 0.0.0.0:8025
BORING_MAIL_BIND=127.0.0.1:9000 cargo run  # Custom bind

# Lint
cargo clippy                   # Lint
cargo fmt                      # Format
```

## Using the Mail Server (Dogfooding)

The server runs on `http://127.0.0.1:8025` (or `http://100.88.146.25:8025` from other machines).
Agent tokens are saved in `/tmp/bms-dogfood/tokens.env`.

### Quick Start
```bash
# Start server (data in /tmp/bms-dogfood)
mkdir -p /tmp/bms-dogfood
BORING_MAIL_BIND=127.0.0.1:8025 BORING_MAIL_DATA=/tmp/bms-dogfood ./target/release/boring-mail serve &

# Register an account
curl -s -X POST http://127.0.0.1:8025/api/accounts \
  -H "Content-Type: application/json" \
  -d '{"name":"my-agent"}' | jq .
# Save the bearerToken from the response
```

### Common Operations
```bash
BMS=http://127.0.0.1:8025
TOKEN=<your-bearer-token>

# Send a message
curl -s -X POST "$BMS/api/messages/send" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"to":["<recipient-id>"],"subject":"Hello","body":"Message body"}'

# Read inbox
curl -s "$BMS/api/messages?label=INBOX" -H "Authorization: Bearer $TOKEN"

# Read a specific message (auto-removes UNREAD)
curl -s "$BMS/api/messages/<msg-id>" -H "Authorization: Bearer $TOKEN"

# Reply in a thread
curl -s -X POST "$BMS/api/messages/send" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"to":["<recipient-id>"],"subject":"RE: Hello","body":"Reply","thread_id":"<thread-id>","in_reply_to":"<msg-id>"}'

# Search messages
curl -s "$BMS/api/search?q=keyword" -H "Authorization: Bearer $TOKEN"

# List labels with counts
curl -s "$BMS/api/labels" -H "Authorization: Bearer $TOKEN"

# Modify labels (add STARRED, remove UNREAD)
curl -s -X POST "$BMS/api/messages/<msg-id>/modify" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"addLabelIds":["STARRED"],"removeLabelIds":["UNREAD"]}'

# List threads
curl -s "$BMS/api/threads?label=INBOX" -H "Authorization: Bearer $TOKEN"
```

### Key Features for Agents
- Every response includes `_diagnostics` with unread count + pending replies
- `x-request-id` UUID header on every response for tracing
- Mailing lists: `POST /api/lists`, send to `list:name` prefix to fan out
- Blob attachments: `POST /api/blobs` then reference hash in message `attachments` field
- Request/response: set `reply_requested: true` and `reply_by` ISO timestamp

### Current Accounts (from /tmp/bms-dogfood/tokens.env)
| Name | Purpose |
|------|---------|
| bms-chief | Orchestrator / team lead |
| bms-researcher | Web research agent |
| bms-fuzzer | Fuzzing agent |
| bms-reviewer | Code review agent |
| bms-perf | Performance benchmarking agent |
| bms-feature | Feature implementation agent |

## Architecture

See MISSION.md for full architecture. Key points:
- **3 storage traits**: DataStore, BlobStore, SearchStore (in `src/storage/mod.rs`)
- **Service layer**: business logic in `src/service/`
- **API layer**: axum handlers in `src/api/`
- **SQLite**: single DB at `~/.boring-mail/mail.db`, WAL mode
- **Blobs**: content-addressed at `~/.boring-mail/blobs/{sha256}.zst`

## Project Files

| File | Purpose |
|------|---------|
| `MISSION.md` | Full architecture, design decisions, agent lifecycle patterns |
| `ROADMAP.md` | Phased task list |
| `PROGRESS.md` | Cycle-by-cycle log |
| `docs/research/` | Industry research from web searches |

## Conventions

- Every commit must pass `cargo test`
- TDD: write test first, then implement
- Functions < 50 lines
- Per-module error types with `thiserror`
- Every API response includes `_diagnostics` (unread count, pending replies)
