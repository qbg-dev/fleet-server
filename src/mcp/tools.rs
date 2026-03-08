//! MCP tool definitions — thin proxies to the boring-mail HTTP API

use reqwest::Client;
use serde_json::{json, Value};

pub struct ToolHandler {
    client: Client,
    base_url: String,
    token: String,
}

impl ToolHandler {
    pub fn new(base_url: &str, token: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            token: token.to_string(),
        }
    }

    pub async fn call(&self, name: &str, args: &Value) -> Result<Value, String> {
        match name {
            "send_message" => self.send_message(args).await,
            "read_inbox" => self.read_inbox(args).await,
            "get_message" => self.get_message(args).await,
            "search_messages" => self.search_messages(args).await,
            "modify_labels" => self.modify_labels(args).await,
            "trash_message" => self.trash_message(args).await,
            "list_labels" => self.list_labels(args).await,
            "list_threads" => self.list_threads(args).await,
            "get_thread" => self.get_thread(args).await,
            _ => Err(format!("unknown tool: {name}")),
        }
    }

    async fn send_message(&self, args: &Value) -> Result<Value, String> {
        let body = json!({
            "to": args.get("to").cloned().unwrap_or(json!([])),
            "cc": args.get("cc").cloned().unwrap_or(json!([])),
            "subject": str_arg(args, "subject")?,
            "body": str_arg(args, "body")?,
            "thread_id": args.get("thread_id").cloned().unwrap_or(Value::Null),
            "in_reply_to": args.get("in_reply_to").cloned().unwrap_or(Value::Null),
            "reply_by": args.get("reply_by").cloned().unwrap_or(Value::Null),
            "labels": args.get("labels").cloned().unwrap_or(json!([])),
            "source": args.get("source").cloned().unwrap_or(json!("mcp")),
            "attachments": args.get("attachments").cloned().unwrap_or(json!([])),
        });

        let resp = self.post("/api/messages/send", &body).await?;
        Ok(text_content(&serde_json::to_string_pretty(&resp).unwrap_or_default()))
    }

    async fn read_inbox(&self, args: &Value) -> Result<Value, String> {
        let label = args.get("label").and_then(|v| v.as_str()).unwrap_or("INBOX");
        let max = args.get("maxResults").and_then(|v| v.as_u64()).unwrap_or(20);
        let mut url = format!("/api/messages?label={label}&maxResults={max}");
        if let Some(token) = args.get("pageToken").and_then(|v| v.as_str()) {
            url.push_str(&format!("&pageToken={token}"));
        }

        let resp = self.get(&url).await?;
        Ok(text_content(&serde_json::to_string_pretty(&resp).unwrap_or_default()))
    }

    async fn get_message(&self, args: &Value) -> Result<Value, String> {
        let id = str_arg(args, "id")?;
        let resp = self.get(&format!("/api/messages/{id}")).await?;
        Ok(text_content(&serde_json::to_string_pretty(&resp).unwrap_or_default()))
    }

    async fn search_messages(&self, args: &Value) -> Result<Value, String> {
        let query = str_arg(args, "q")?;
        let max = args.get("maxResults").and_then(|v| v.as_u64()).unwrap_or(20);
        let encoded = urlencoding::encode(&query);
        let resp = self.get(&format!("/api/search?q={encoded}&maxResults={max}")).await?;
        Ok(text_content(&serde_json::to_string_pretty(&resp).unwrap_or_default()))
    }

    async fn modify_labels(&self, args: &Value) -> Result<Value, String> {
        let id = str_arg(args, "id")?;
        let body = json!({
            "addLabelIds": args.get("addLabelIds").cloned().unwrap_or(json!([])),
            "removeLabelIds": args.get("removeLabelIds").cloned().unwrap_or(json!([])),
        });
        let resp = self.post(&format!("/api/messages/{id}/modify"), &body).await?;
        Ok(text_content(&serde_json::to_string_pretty(&resp).unwrap_or_default()))
    }

    async fn trash_message(&self, args: &Value) -> Result<Value, String> {
        let id = str_arg(args, "id")?;
        let resp = self.post(&format!("/api/messages/{id}/trash"), &json!({})).await?;
        Ok(text_content(&serde_json::to_string_pretty(&resp).unwrap_or_default()))
    }

    async fn list_labels(&self, _args: &Value) -> Result<Value, String> {
        let resp = self.get("/api/labels").await?;
        Ok(text_content(&serde_json::to_string_pretty(&resp).unwrap_or_default()))
    }

    async fn list_threads(&self, args: &Value) -> Result<Value, String> {
        let label = args.get("label").and_then(|v| v.as_str()).unwrap_or("INBOX");
        let max = args.get("maxResults").and_then(|v| v.as_u64()).unwrap_or(20);
        let resp = self.get(&format!("/api/threads?label={label}&maxResults={max}")).await?;
        Ok(text_content(&serde_json::to_string_pretty(&resp).unwrap_or_default()))
    }

    async fn get_thread(&self, args: &Value) -> Result<Value, String> {
        let id = str_arg(args, "id")?;
        let resp = self.get(&format!("/api/threads/{id}")).await?;
        Ok(text_content(&serde_json::to_string_pretty(&resp).unwrap_or_default()))
    }

    // HTTP helpers

    async fn get(&self, path: &str) -> Result<Value, String> {
        let url = format!("{}{path}", self.base_url);
        let resp = self.client
            .get(&url)
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|e| format!("HTTP GET {path} failed: {e}"))?;

        let status = resp.status();
        let body = resp.text().await.map_err(|e| format!("read body failed: {e}"))?;

        if !status.is_success() {
            return Err(format!("HTTP {status}: {body}"));
        }

        serde_json::from_str(&body).map_err(|e| format!("parse JSON failed: {e}"))
    }

    async fn post(&self, path: &str, body: &Value) -> Result<Value, String> {
        let url = format!("{}{path}", self.base_url);
        let resp = self.client
            .post(&url)
            .bearer_auth(&self.token)
            .json(body)
            .send()
            .await
            .map_err(|e| format!("HTTP POST {path} failed: {e}"))?;

        let status = resp.status();
        let body_text = resp.text().await.map_err(|e| format!("read body failed: {e}"))?;

        if !status.is_success() {
            return Err(format!("HTTP {status}: {body_text}"));
        }

        serde_json::from_str(&body_text).map_err(|e| format!("parse JSON failed: {e}"))
    }
}

fn str_arg(args: &Value, key: &str) -> Result<String, String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("missing required argument: {key}"))
}

fn text_content(text: &str) -> Value {
    json!({
        "content": [{"type": "text", "text": text}],
    })
}

pub fn list_tools() -> Value {
    json!({
        "tools": [
            {
                "name": "send_message",
                "description": "Send a message to one or more recipients. Use 'list:name' prefix for mailing lists.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "to": {"type": "array", "items": {"type": "string"}, "description": "Recipient account names or 'list:name' for mailing lists"},
                        "cc": {"type": "array", "items": {"type": "string"}, "description": "CC recipients"},
                        "subject": {"type": "string", "description": "Message subject"},
                        "body": {"type": "string", "description": "Message body"},
                        "thread_id": {"type": "string", "description": "Thread ID to reply in"},
                        "in_reply_to": {"type": "string", "description": "Message ID being replied to"},
                        "reply_by": {"type": "string", "description": "ISO timestamp deadline for reply"},
                        "labels": {"type": "array", "items": {"type": "string"}, "description": "Additional labels"},
                        "attachments": {"type": "array", "items": {"type": "string"}, "description": "Blob hashes to attach"},
                    },
                    "required": ["to", "subject", "body"],
                },
            },
            {
                "name": "read_inbox",
                "description": "List messages by label (default: INBOX). Returns message summaries with pagination.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "label": {"type": "string", "description": "Label to filter by (default: INBOX)"},
                        "maxResults": {"type": "integer", "description": "Max messages to return (default: 20)"},
                        "pageToken": {"type": "string", "description": "Pagination token from previous response"},
                    },
                },
            },
            {
                "name": "get_message",
                "description": "Get full message details by ID. Auto-removes UNREAD label.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "id": {"type": "string", "description": "Message ID"},
                    },
                    "required": ["id"],
                },
            },
            {
                "name": "search_messages",
                "description": "Full-text search with Gmail query syntax (from:, to:, subject:, has:attachment, label:, date ranges).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "q": {"type": "string", "description": "Search query using Gmail syntax"},
                        "maxResults": {"type": "integer", "description": "Max results (default: 20)"},
                    },
                    "required": ["q"],
                },
            },
            {
                "name": "modify_labels",
                "description": "Add or remove labels on a message.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "id": {"type": "string", "description": "Message ID"},
                        "addLabelIds": {"type": "array", "items": {"type": "string"}, "description": "Labels to add"},
                        "removeLabelIds": {"type": "array", "items": {"type": "string"}, "description": "Labels to remove"},
                    },
                    "required": ["id"],
                },
            },
            {
                "name": "trash_message",
                "description": "Move a message to trash (removes INBOX, adds TRASH label).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "id": {"type": "string", "description": "Message ID"},
                    },
                    "required": ["id"],
                },
            },
            {
                "name": "list_labels",
                "description": "List all labels with message counts and unread counts.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                },
            },
            {
                "name": "list_threads",
                "description": "List conversation threads by label.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "label": {"type": "string", "description": "Label to filter by (default: INBOX)"},
                        "maxResults": {"type": "integer", "description": "Max threads (default: 20)"},
                    },
                },
            },
            {
                "name": "get_thread",
                "description": "Get a thread with all its messages.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "id": {"type": "string", "description": "Thread ID"},
                    },
                    "required": ["id"],
                },
            },
        ]
    })
}
