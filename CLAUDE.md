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
