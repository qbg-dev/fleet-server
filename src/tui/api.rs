use serde::{Deserialize, Serialize};
use tokio_tungstenite::tungstenite;

/// HTTP + WebSocket client for boring-mail server.
pub struct ApiClient {
    pub base_url: String,
    pub token: String,
    http: reqwest::Client,
}

// ── API response types ──────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct ThreadSummary {
    pub id: String,
    pub subject: String,
    pub snippet: String,
    #[serde(rename = "lastMessageAt")]
    pub last_message_at: String,
    #[serde(rename = "messageCount")]
    pub message_count: u32,
    pub participants: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MessageSummary {
    pub id: String,
    #[serde(rename = "threadId")]
    pub thread_id: String,
    pub from: String,
    pub subject: String,
    pub snippet: String,
    #[serde(rename = "internalDate")]
    pub internal_date: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FullMessage {
    pub id: String,
    #[serde(rename = "threadId")]
    pub thread_id: String,
    pub from: String,
    pub to: Vec<String>,
    #[serde(default)]
    pub cc: Vec<String>,
    pub subject: String,
    pub body: String,
    #[serde(rename = "labelIds", default)]
    pub label_ids: Vec<String>,
    #[serde(rename = "internalDate")]
    pub internal_date: String,
    #[serde(rename = "inReplyTo")]
    pub in_reply_to: Option<String>,
    #[serde(rename = "replyBy")]
    pub reply_by: Option<String>,
    #[serde(rename = "replyRequested", default)]
    pub reply_requested: bool,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThreadDetail {
    pub id: String,
    pub subject: String,
    pub snippet: String,
    #[serde(rename = "lastMessageAt")]
    pub last_message_at: String,
    #[serde(rename = "messageCount")]
    pub message_count: u32,
    pub participants: Vec<String>,
    pub messages: Vec<FullMessage>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LabelInfo {
    pub name: String,
    #[serde(rename = "type")]
    pub label_type: String,
    #[serde(rename = "messageCount")]
    pub message_count: u32,
    #[serde(rename = "unreadCount")]
    pub unread_count: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DirectoryEntry {
    pub id: String,
    pub name: String,
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
    pub bio: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SendRequest {
    pub to: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub cc: Vec<String>,
    pub subject: String,
    pub body: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_reply_to: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WsEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub account_id: Option<String>,
    pub message_id: Option<String>,
    pub from: Option<String>,
    pub subject: Option<String>,
    pub thread_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AccountInfo {
    pub id: String,
    pub name: String,
}

// ── HTTP Client ─────────────────────────────────────────────────────

impl ApiClient {
    pub fn new(base_url: &str, token: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            token: token.to_string(),
            http: reqwest::Client::new(),
        }
    }

    fn auth(&self) -> String {
        format!("Bearer {}", self.token)
    }

    pub async fn get_me(&self) -> anyhow::Result<AccountInfo> {
        let resp: serde_json::Value = self.http
            .get(format!("{}/api/accounts/me", self.base_url))
            .header("Authorization", self.auth())
            .send().await?
            .json().await?;
        Ok(AccountInfo {
            id: resp["id"].as_str().unwrap_or_default().to_string(),
            name: resp["name"].as_str().unwrap_or_default().to_string(),
        })
    }

    pub async fn list_threads(&self, label: &str, max_results: u32) -> anyhow::Result<Vec<ThreadSummary>> {
        let resp: serde_json::Value = self.http
            .get(format!("{}/api/threads", self.base_url))
            .query(&[("label", label), ("maxResults", &max_results.to_string())])
            .header("Authorization", self.auth())
            .send().await?
            .json().await?;
        let threads: Vec<ThreadSummary> = serde_json::from_value(
            resp.get("threads").cloned().unwrap_or(serde_json::Value::Array(vec![]))
        )?;
        Ok(threads)
    }

    pub async fn get_thread(&self, id: &str) -> anyhow::Result<ThreadDetail> {
        let resp: ThreadDetail = self.http
            .get(format!("{}/api/threads/{}", self.base_url, id))
            .header("Authorization", self.auth())
            .send().await?
            .json().await?;
        Ok(resp)
    }

    pub async fn get_message(&self, id: &str) -> anyhow::Result<FullMessage> {
        let resp: FullMessage = self.http
            .get(format!("{}/api/messages/{}", self.base_url, id))
            .header("Authorization", self.auth())
            .send().await?
            .json().await?;
        Ok(resp)
    }

    pub async fn list_labels(&self) -> anyhow::Result<Vec<LabelInfo>> {
        let resp: serde_json::Value = self.http
            .get(format!("{}/api/labels", self.base_url))
            .header("Authorization", self.auth())
            .send().await?
            .json().await?;
        let labels: Vec<LabelInfo> = serde_json::from_value(
            resp.get("labels").cloned().unwrap_or(serde_json::Value::Array(vec![]))
        )?;
        Ok(labels)
    }

    pub async fn send_message(&self, req: &SendRequest) -> anyhow::Result<serde_json::Value> {
        let resp: serde_json::Value = self.http
            .post(format!("{}/api/messages/send", self.base_url))
            .header("Authorization", self.auth())
            .json(req)
            .send().await?
            .json().await?;
        Ok(resp)
    }

    pub async fn modify_labels(&self, msg_id: &str, add: &[&str], remove: &[&str]) -> anyhow::Result<()> {
        self.http
            .post(format!("{}/api/messages/{}/modify", self.base_url, msg_id))
            .header("Authorization", self.auth())
            .json(&serde_json::json!({
                "addLabelIds": add,
                "removeLabelIds": remove,
            }))
            .send().await?;
        Ok(())
    }

    pub async fn trash_message(&self, msg_id: &str) -> anyhow::Result<()> {
        self.http
            .post(format!("{}/api/messages/{}/trash", self.base_url, msg_id))
            .header("Authorization", self.auth())
            .send().await?;
        Ok(())
    }

    pub async fn search(&self, query: &str, max_results: u32) -> anyhow::Result<Vec<String>> {
        let resp: serde_json::Value = self.http
            .get(format!("{}/api/search", self.base_url))
            .query(&[("q", query), ("maxResults", &max_results.to_string())])
            .header("Authorization", self.auth())
            .send().await?
            .json().await?;
        let ids: Vec<String> = serde_json::from_value(
            resp.get("messageIds").cloned().unwrap_or(serde_json::Value::Array(vec![]))
        )?;
        Ok(ids)
    }

    pub async fn get_directory(&self) -> anyhow::Result<Vec<DirectoryEntry>> {
        let resp: serde_json::Value = self.http
            .get(format!("{}/api/directory", self.base_url))
            .header("Authorization", self.auth())
            .send().await?
            .json().await?;
        let entries: Vec<DirectoryEntry> = serde_json::from_value(
            resp.get("directory").cloned().unwrap_or(serde_json::Value::Array(vec![]))
        )?;
        Ok(entries)
    }
}

// ── WebSocket ───────────────────────────────────────────────────────

pub async fn connect_ws(base_url: &str, token: &str) -> anyhow::Result<
    futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>
        >
    >
> {
    let ws_url = base_url.replacen("http", "ws", 1);
    let url = format!("{}/ws?token={}", ws_url, token);
    let (ws_stream, _) = tokio_tungstenite::connect_async(&url).await?;
    let (_write, read) = futures_util::StreamExt::split(ws_stream);
    Ok(read)
}

pub fn parse_ws_event(msg: &tungstenite::Message) -> Option<WsEvent> {
    match msg {
        tungstenite::Message::Text(text) => serde_json::from_str(text.as_ref()).ok(),
        _ => None,
    }
}
