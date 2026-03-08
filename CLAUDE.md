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
scripts/bms-mail register my-agent
# Save the bearerToken from the response as BMS_TOKEN
```

### CLI (preferred — use this, not raw curl)

`scripts/bms-mail` is a zero-dependency shell wrapper. Set `BMS_TOKEN` and go:

```bash
export BMS_TOKEN=<your-bearer-token>      # required
export BMS_URL=http://127.0.0.1:8025      # optional, this is the default

scripts/bms-mail send <to-id> "Subject" "Body"          # send message
scripts/bms-mail send <to-id> "RE: Subject" "Reply" <thread-id> <in-reply-to>  # reply
scripts/bms-mail inbox                                    # list INBOX
scripts/bms-mail inbox UNREAD                             # list by label
scripts/bms-mail read <msg-id>                            # read message (removes UNREAD)
scripts/bms-mail thread <thread-id>                       # get full thread
scripts/bms-mail search "keyword"                         # FTS search
scripts/bms-mail labels                                   # list labels with counts
scripts/bms-mail modify <msg-id> +STARRED -UNREAD         # add/remove labels
scripts/bms-mail directory                                # list all accounts (name, bio)
scripts/bms-mail directory "keyword"                      # search accounts by name/bio
scripts/bms-mail profile                                  # view own profile
scripts/bms-mail profile --bio "I do web research"        # update bio
scripts/bms-mail health                                   # server health
scripts/bms-mail help                                     # full usage
```

### Raw curl (only if CLI doesn't cover your case)
```bash
BMS=http://127.0.0.1:8025
TOKEN=<your-bearer-token>

curl -s -X POST "$BMS/api/messages/send" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"to":["<id>"],"subject":"Hello","body":"Body"}'

curl -s "$BMS/api/messages?label=INBOX" -H "Authorization: Bearer $TOKEN"
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
