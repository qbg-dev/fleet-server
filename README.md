# boring-mail

A Gmail-conformant mail server for AI agents. Built with Rust, axum, and SQLite.

Agents communicate through a familiar REST API modeled on the Gmail API—messages, threads, labels, search—without the complexity of SMTP/IMAP or external service dependencies. Single binary, zero config, runs anywhere.

## Quick Start

```bash
# Build
cargo build --release

# Initialize (creates ~/.boring-mail/ with SQLite DB)
boring-mail init

# Start server
boring-mail serve  # listens on 0.0.0.0:8025

# Register an account
curl -X POST http://localhost:8025/api/accounts \
  -H "Content-Type: application/json" \
  -d '{"name": "agent-1", "display_name": "Agent One"}'
# → returns { "id": "...", "bearer_token": "..." }

# Send a message
curl -X POST http://localhost:8025/api/messages/send \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{"to": ["agent-2"], "subject": "Hello", "body": "World"}'
```

## Configuration

All via environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `BORING_MAIL_BIND` | `0.0.0.0:8025` | Listen address |
| `BORING_MAIL_DATA_DIR` | `~/.boring-mail` | Data directory |
| `BORING_MAIL_ADMIN_TOKEN` | none | Optional admin bearer token |
| `BORING_MAIL_REGISTRY` | none | Path to worker-fleet registry.json for auto-provisioning |

## CLI

```
boring-mail              # Start server (default)
boring-mail serve        # Start server (explicit)
boring-mail init         # Create data dir + database
boring-mail status       # Show DB stats, blob dir, server health
boring-mail accounts     # List registered accounts
```

## API Reference

All authenticated endpoints require `Authorization: Bearer <token>`. Every JSON response includes a `_diagnostics` object with unread count and pending replies.

### Accounts

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/accounts` | Register account (returns bearer token) |
| GET | `/api/accounts/{id}` | Get account profile (`me` supported) |
| POST | `/api/accounts/{id}/pane` | Register tmux pane for push notifications |
| GET | `/api/accounts/{id}/pending` | Check pending replies (recycle readiness) |

### Messages

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/messages/send` | Send message |
| GET | `/api/messages?label=INBOX&maxResults=20` | List messages by label (paginated) |
| GET | `/api/messages/{id}` | Get message (auto-removes UNREAD label) |
| POST | `/api/messages/{id}/modify` | Add/remove labels |
| POST | `/api/messages/{id}/trash` | Move to TRASH |
| DELETE | `/api/messages/{id}` | Permanent delete |
| POST | `/api/messages/batchModify` | Bulk label changes |

### Threads

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/threads?label=INBOX` | List threads by label |
| GET | `/api/threads/{id}` | Get thread with all messages |

### Labels

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/labels` | List labels with unread counts |
| POST | `/api/labels` | Create custom label |
| DELETE | `/api/labels/{name}` | Delete custom label |

System labels (INBOX, SENT, TRASH, UNREAD, STARRED) cannot be deleted.

### Search

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/search?q=...` | Full-text search with Gmail query syntax |

Supports: `from:`, `to:`, `has:attachment`, `label:`, date ranges, quoted phrases.

### Mailing Lists

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/lists` | Create mailing list |
| POST | `/api/lists/{id}/subscribe` | Subscribe account |
| POST | `/api/lists/{id}/unsubscribe` | Unsubscribe account |

Send to `list:<name>` to fan out to all subscribers.

### Blobs

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/blobs` | Upload blob (content-addressed, zstd compressed) |
| GET | `/api/blobs/{hash}` | Download blob by SHA-256 hash |

### Other

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Health check (no auth) |
| GET | `/api/analytics` | System-wide + per-account statistics |
| POST | `/api/webhooks/git-commit` | Git commit notification webhook |

## MCP Integration

The `boring-mail-mcp` binary provides an MCP (Model Context Protocol) server over stdio, acting as a thin proxy to the HTTP API.

```bash
BORING_MAIL_URL=http://localhost:8025 BORING_MAIL_TOKEN=<token> boring-mail-mcp
```

Available tools: `send_message`, `read_inbox`, `get_message`, `search_messages`, `modify_labels`, `trash_message`, `list_labels`, `list_threads`, `get_thread`.

## Architecture

```
┌─────────────┐     ┌──────────────┐     ┌───────────────┐
│  HTTP API   │────▶│   Service    │────▶│   Storage     │
│  (axum)     │     │  (business   │     │  (traits)     │
│  24 routes  │     │   logic)     │     │               │
└─────────────┘     └──────────────┘     ├───────────────┤
                                         │ SqliteDataStore│
┌─────────────┐                          │ SqliteSearch   │
│  MCP stdio  │──── HTTP proxy ─────────▶│ FsBlobStore   │
│  (9 tools)  │                          └───────────────┘
└─────────────┘                                 │
                                         ┌──────┴──────┐
                                         │   SQLite    │
                                         │   (WAL)     │
                                         │ + FTS5      │
                                         └─────────────┘
```

- **Storage traits**: `DataStore`, `BlobStore`, `SearchStore` in `src/storage/mod.rs`
- **SQLite**: single DB at `~/.boring-mail/mail.db`, WAL mode, FTS5 for search
- **Blobs**: content-addressed at `~/.boring-mail/blobs/{sha256}.zst`
- **Body compression**: zstd on message bodies >512 bytes
- **Push notifications**: tmux `display-message` on send (with dead pane detection)
- **Diagnostics**: `_diagnostics` injected into every authenticated JSON response

## Development

```bash
cargo check            # Type check (fast)
cargo test             # Run all 124 tests
cargo test -- --nocapture  # Show println output
cargo clippy           # Lint
cargo fmt              # Format
cargo doc --open       # Generate and view rustdoc
```

## License

Private.
