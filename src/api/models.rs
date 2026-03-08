// API request/response types — Phase 2
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct SendMessageRequest {
    pub to: Vec<String>,
    #[serde(default)]
    pub cc: Vec<String>,
    pub subject: String,
    pub body: String,
    pub thread_id: Option<String>,
    pub in_reply_to: Option<String>,
    pub reply_by: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    pub source: Option<String>,
    #[serde(default)]
    pub attachments: Vec<String>, // blob hashes
}

#[derive(Debug, Deserialize)]
pub struct ModifyLabelsRequest {
    #[serde(default, rename = "addLabelIds")]
    pub add_label_ids: Vec<String>,
    #[serde(default, rename = "removeLabelIds")]
    pub remove_label_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct BatchModifyRequest {
    pub ids: Vec<String>,
    #[serde(default, rename = "addLabelIds")]
    pub add_label_ids: Vec<String>,
    #[serde(default, rename = "removeLabelIds")]
    pub remove_label_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateAccountRequest {
    pub name: String,
    pub display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub label: Option<String>,
    #[serde(default = "default_max_results", rename = "maxResults")]
    pub max_results: u32,
    #[serde(rename = "pageToken")]
    pub page_token: Option<String>,
    pub q: Option<String>,
}

fn default_max_results() -> u32 {
    20
}

#[derive(Debug, Serialize)]
pub struct Diagnostics {
    pub unread_count: u32,
    pub pending_replies: Vec<crate::storage::models::PendingReply>,
    pub overdue_count: u32,
    pub inbox_hint: Option<String>,
}
