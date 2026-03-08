use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub name: String,
    pub display_name: Option<String>,
    pub bearer_token: String,
    pub tmux_pane_id: Option<String>,
    pub active: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub thread_id: String,
    pub from_account: String,
    pub subject: String,
    pub body: String,
    pub snippet: String,
    pub has_attachments: bool,
    pub internal_date: String,
    pub in_reply_to: Option<String>,
    pub reply_by: Option<String>,
    pub reply_requested: bool,
    pub labels: Vec<String>,
    pub recipients: Vec<Recipient>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recipient {
    pub account_id: String,
    pub recipient_type: String,
}

#[derive(Debug, Clone)]
pub struct NewMessage {
    pub from_account: String,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub subject: String,
    pub body: String,
    pub thread_id: Option<String>,
    pub in_reply_to: Option<String>,
    pub reply_by: Option<String>,
    pub labels: Vec<String>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageList {
    pub messages: Vec<Message>,
    pub next_page_token: Option<String>,
    pub result_size_estimate: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thread {
    pub id: String,
    pub subject: String,
    pub snippet: String,
    pub last_message_at: String,
    pub message_count: u32,
    pub participants: Vec<String>,
    pub messages: Vec<Message>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadList {
    pub threads: Vec<Thread>,
    pub next_page_token: Option<String>,
    pub result_size_estimate: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabelCount {
    pub name: String,
    pub label_type: String,
    pub message_count: u32,
    pub unread_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlobMeta {
    pub hash: String,
    pub size: u64,
    pub compressed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingReply {
    pub message_id: String,
    pub from_account: String,
    pub subject: String,
    pub reply_by: Option<String>,
    pub sent_at: String,
}
