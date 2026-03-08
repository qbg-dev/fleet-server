//! # boring-mail
//!
//! A Gmail-conformant mail server for AI agents, built with axum and SQLite.
//!
//! ## Architecture
//!
//! - **[`storage`]**: Core traits (`DataStore`, `BlobStore`, `SearchStore`) and implementations
//! - **[`service`]**: Business logic for accounts, messages, labels, threads, mailing lists
//! - **[`api`]**: HTTP handlers (24 endpoints) with auth middleware and diagnostics
//! - **[`search`]**: FTS5 indexing with Gmail query syntax parser
//! - **[`delivery`]**: Push notification backends (tmux)
//! - **[`background`]**: Long-running tasks (deadline checking)
//! - **[`db`]**: SQLite connection pool and schema migrations
//! - **[`config`]**: Environment-based configuration
//! - **[`error`]**: Layered error types (`StorageError` → `MessageError` → `ApiError`)

pub mod config;
pub mod db;
pub mod error;
pub mod storage;
pub mod api;
pub mod service;
pub mod search;
pub mod delivery;
pub mod background;
