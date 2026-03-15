//! Core data models shared across storage, service, and API layers.

use serde::{Deserialize, Serialize};

/// A registered mail account with authentication credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    /// Unique short name (used in addressing, e.g. `agent-1`).
    pub name: String,
    pub display_name: Option<String>,
    /// Free-text description of this account's role/capabilities.
    pub bio: Option<String>,
    /// Bearer token for API authentication.
    pub bearer_token: String,
    /// tmux pane ID for push notifications (e.g. `%42`).
    pub tmux_pane_id: Option<String>,
    pub active: bool,
    pub created_at: String,
    /// SHA-256 hash of the latest session file blob.
    pub session_blob_hash: Option<String>,
    /// ISO timestamp of last session file sync.
    pub session_synced_at: Option<String>,
}

/// A stored message with resolved labels and recipients.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub thread_id: String,
    /// Account name of the sender.
    pub from_account: String,
    pub subject: String,
    /// Full message body (transparently decompressed if stored with zstd).
    pub body: String,
    /// First 100 chars of body, for list views.
    pub snippet: String,
    pub has_attachments: bool,
    /// Microsecond-precision ISO 8601 timestamp.
    pub internal_date: String,
    /// Message ID this is replying to.
    pub in_reply_to: Option<String>,
    /// Deadline for reply (ISO 8601). Messages past this get labeled OVERDUE.
    pub reply_by: Option<String>,
    pub reply_requested: bool,
    /// Labels for the authenticated account (e.g. INBOX, UNREAD, SENT).
    pub labels: Vec<String>,
    pub recipients: Vec<Recipient>,
    /// Optional source tag (e.g. "git-webhook", "mailing-list").
    pub source: Option<String>,
}

/// A message recipient with type (TO or CC).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recipient {
    pub account_id: String,
    /// `"TO"` or `"CC"`.
    pub recipient_type: String,
}

/// Input for creating a new message via [`super::DataStore::insert_message`].
#[derive(Debug, Clone, Default)]
pub struct NewMessage {
    pub from_account: String,
    /// Recipient account names. Use `list:<name>` for mailing list fan-out.
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub subject: String,
    pub body: String,
    /// Explicit thread ID, or auto-resolved from `in_reply_to`.
    pub thread_id: Option<String>,
    pub in_reply_to: Option<String>,
    pub reply_by: Option<String>,
    pub labels: Vec<String>,
    pub source: Option<String>,
    /// SHA-256 hashes of blobs to attach.
    pub attachments: Vec<String>,
}

/// Paginated list of messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageList {
    pub messages: Vec<Message>,
    /// Opaque token for the next page, or `None` if this is the last page.
    pub next_page_token: Option<String>,
    pub result_size_estimate: u32,
}

/// A conversation thread grouping related messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thread {
    pub id: String,
    /// Subject of the first message in the thread.
    pub subject: String,
    /// Snippet from the most recent message.
    pub snippet: String,
    pub last_message_at: String,
    pub message_count: u32,
    /// Account names of all participants.
    pub participants: Vec<String>,
    /// All messages in chronological order (populated in thread get, empty in list).
    pub messages: Vec<Message>,
}

/// Paginated list of threads.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadList {
    pub threads: Vec<Thread>,
    pub next_page_token: Option<String>,
    pub result_size_estimate: u32,
}

/// Label with message and unread counts for an account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabelCount {
    pub name: String,
    /// `"system"` or `"user"`.
    pub label_type: String,
    pub message_count: u32,
    pub unread_count: u32,
}

/// Metadata for a blob attached to a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub blob_hash: String,
    pub filename: String,
    pub content_type: String,
    pub size: u64,
}

/// Metadata returned after storing a blob.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlobMeta {
    /// SHA-256 hex digest of the original content.
    pub hash: String,
    /// Original (uncompressed) size in bytes.
    pub size: u64,
    /// Whether the blob was stored with zstd compression.
    pub compressed: bool,
}

/// A message awaiting a reply, used for recycle readiness checks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingReply {
    pub message_id: String,
    pub from_account: String,
    pub subject: String,
    pub reply_by: Option<String>,
    pub sent_at: String,
}

/// Per-account statistics for the analytics endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountStats {
    pub account_id: String,
    pub account_name: String,
    pub messages_sent: u32,
    pub messages_received: u32,
    pub threads_started: u32,
    pub unread_count: u32,
}

/// System-wide analytics with per-account breakdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Analytics {
    pub total_accounts: u32,
    pub total_messages: u32,
    pub total_threads: u32,
    pub total_blobs: u32,
    pub per_account: Vec<AccountStats>,
}
