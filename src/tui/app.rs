use crate::api::{ApiClient, FullMessage, LabelInfo, SendRequest, ThreadDetail, ThreadSummary};

// ── Modes ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    /// Browsing thread list (left pane focused)
    Normal,
    /// Reading a thread (right pane focused)
    Reading,
    /// Composing or replying
    Compose,
    /// Search input
    Search,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ComposeField {
    To,
    Cc,
    Subject,
    Body,
}

// ── App State ───────────────────────────────────────────────────────

pub struct App {
    pub mode: Mode,
    pub current_label: String,
    pub threads: Vec<ThreadSummary>,
    pub thread_index: usize,
    pub thread_detail: Option<ThreadDetail>,
    pub scroll_offset: u16,
    pub labels: Vec<LabelInfo>,
    pub account_name: String,
    pub account_id: String,

    // Compose state
    pub compose: ComposeState,

    // Search state
    pub search_input: String,
    pub search_results: Vec<FullMessage>,

    // Status
    pub status: String,
    pub should_quit: bool,

    // Namespace to strip from display names
    pub namespace: Option<String>,
}

pub struct ComposeState {
    pub to: String,
    pub cc: String,
    pub subject: String,
    pub body: Vec<String>, // lines
    pub cursor_line: usize,
    pub cursor_col: usize,
    pub field: ComposeField,
    pub in_reply_to: Option<String>,
    pub thread_id: Option<String>,
}

impl ComposeState {
    pub fn new() -> Self {
        Self {
            to: String::new(),
            cc: String::new(),
            subject: String::new(),
            body: vec![String::new()],
            cursor_line: 0,
            cursor_col: 0,
            field: ComposeField::To,
            in_reply_to: None,
            thread_id: None,
        }
    }

    pub fn clear(&mut self) {
        self.to.clear();
        self.cc.clear();
        self.subject.clear();
        self.body = vec![String::new()];
        self.cursor_line = 0;
        self.cursor_col = 0;
        self.field = ComposeField::To;
        self.in_reply_to = None;
        self.thread_id = None;
    }

    pub fn body_text(&self) -> String {
        self.body.join("\n")
    }

    /// Get mutable ref to the active single-line field
    pub fn active_field_mut(&mut self) -> Option<&mut String> {
        match self.field {
            ComposeField::To => Some(&mut self.to),
            ComposeField::Cc => Some(&mut self.cc),
            ComposeField::Subject => Some(&mut self.subject),
            ComposeField::Body => None,
        }
    }
}

impl App {
    pub fn new(account_name: String, account_id: String) -> Self {
        // Auto-detect namespace from account name
        let namespace = account_name.split('@').nth(1).map(|s| format!("@{s}"));
        Self {
            mode: Mode::Normal,
            current_label: "INBOX".to_string(),
            threads: vec![],
            thread_index: 0,
            thread_detail: None,
            scroll_offset: 0,
            labels: vec![],
            account_name,
            account_id,
            compose: ComposeState::new(),
            search_input: String::new(),
            search_results: vec![],
            status: String::new(),
            should_quit: false,
            namespace,
        }
    }

    /// Strip namespace from display name (e.g., "merger@wechat" → "merger")
    pub fn display_name<'a>(&self, name: &'a str) -> &'a str {
        if let Some(ref ns) = self.namespace {
            name.strip_suffix(ns.as_str()).unwrap_or(name)
        } else {
            name
        }
    }

    pub fn selected_thread(&self) -> Option<&ThreadSummary> {
        self.threads.get(self.thread_index)
    }

    pub fn unread_count(&self) -> u32 {
        self.labels.iter()
            .find(|l| l.name == "UNREAD")
            .map(|l| l.message_count)
            .unwrap_or(0)
    }

    pub fn label_unread(&self, name: &str) -> u32 {
        self.labels.iter()
            .find(|l| l.name == name)
            .map(|l| l.unread_count)
            .unwrap_or(0)
    }

    // ── Actions ─────────────────────────────────────────────────────

    pub async fn refresh_threads(&mut self, api: &ApiClient) {
        match api.list_threads(&self.current_label, 50).await {
            Ok(threads) => {
                self.threads = threads;
                if self.thread_index >= self.threads.len() && !self.threads.is_empty() {
                    self.thread_index = self.threads.len() - 1;
                }
            }
            Err(e) => self.status = format!("Error: {e}"),
        }
    }

    pub async fn refresh_labels(&mut self, api: &ApiClient) {
        match api.list_labels().await {
            Ok(labels) => self.labels = labels,
            Err(e) => self.status = format!("Labels error: {e}"),
        }
    }

    pub async fn open_thread(&mut self, api: &ApiClient) {
        if let Some(thread) = self.selected_thread() {
            let tid = thread.id.clone();
            match api.get_thread(&tid).await {
                Ok(detail) => {
                    self.thread_detail = Some(detail);
                    self.scroll_offset = 0;
                    self.mode = Mode::Reading;
                }
                Err(e) => self.status = format!("Error: {e}"),
            }
        }
    }

    pub async fn archive_selected(&mut self, api: &ApiClient) {
        if let Some(detail) = &self.thread_detail {
            for msg in &detail.messages {
                let _ = api.modify_labels(&msg.id, &[], &["INBOX"]).await;
            }
            self.status = "Archived".to_string();
            self.mode = Mode::Normal;
            self.thread_detail = None;
            self.refresh_threads(api).await;
            self.refresh_labels(api).await;
        }
    }

    pub async fn star_selected(&mut self, api: &ApiClient) {
        if let Some(detail) = &self.thread_detail {
            if let Some(msg) = detail.messages.last() {
                let is_starred = msg.label_ids.iter().any(|l| l == "STARRED");
                if is_starred {
                    let _ = api.modify_labels(&msg.id, &[], &["STARRED"]).await;
                    self.status = "Unstarred".to_string();
                } else {
                    let _ = api.modify_labels(&msg.id, &["STARRED"], &[]).await;
                    self.status = "Starred".to_string();
                }
            }
        }
    }

    pub async fn trash_selected(&mut self, api: &ApiClient) {
        if let Some(detail) = &self.thread_detail {
            for msg in &detail.messages {
                let _ = api.trash_message(&msg.id).await;
            }
            self.status = "Trashed".to_string();
            self.mode = Mode::Normal;
            self.thread_detail = None;
            self.refresh_threads(api).await;
            self.refresh_labels(api).await;
        }
    }

    pub fn start_compose(&mut self) {
        self.compose.clear();
        self.mode = Mode::Compose;
    }

    pub fn start_reply(&mut self) {
        if let Some(detail) = &self.thread_detail {
            if let Some(last_msg) = detail.messages.last() {
                self.compose.clear();
                self.compose.to = last_msg.from.clone();
                self.compose.subject = if last_msg.subject.starts_with("Re: ") {
                    last_msg.subject.clone()
                } else {
                    format!("Re: {}", last_msg.subject)
                };
                self.compose.in_reply_to = Some(last_msg.id.clone());
                self.compose.thread_id = Some(detail.id.clone());
                self.compose.field = ComposeField::Body;
                self.mode = Mode::Compose;
            }
        }
    }

    pub async fn send_compose(&mut self, api: &ApiClient) {
        let to: Vec<String> = self.compose.to
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        if to.is_empty() {
            self.status = "No recipients".to_string();
            return;
        }

        let req = SendRequest {
            to,
            cc: self.compose.cc
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            subject: self.compose.subject.clone(),
            body: self.compose.body_text(),
            thread_id: self.compose.thread_id.clone(),
            in_reply_to: self.compose.in_reply_to.clone(),
        };

        match api.send_message(&req).await {
            Ok(_) => {
                self.status = "Sent!".to_string();
                self.compose.clear();
                self.mode = if self.thread_detail.is_some() { Mode::Reading } else { Mode::Normal };
                self.refresh_threads(api).await;
                self.refresh_labels(api).await;
                // Re-open thread to show the new reply
                if let Some(detail) = &self.thread_detail {
                    let tid = detail.id.clone();
                    if let Ok(updated) = api.get_thread(&tid).await {
                        self.thread_detail = Some(updated);
                    }
                }
            }
            Err(e) => self.status = format!("Send failed: {e}"),
        }
    }

    pub async fn switch_label(&mut self, label: &str, api: &ApiClient) {
        self.current_label = label.to_string();
        self.thread_index = 0;
        self.thread_detail = None;
        self.mode = Mode::Normal;
        self.refresh_threads(api).await;
    }

    pub async fn execute_search(&mut self, api: &ApiClient) {
        let query = self.search_input.clone();
        if query.is_empty() {
            self.mode = Mode::Normal;
            return;
        }
        match api.search(&query, 20).await {
            Ok(ids) => {
                let mut results = Vec::new();
                for id in ids.iter().take(20) {
                    if let Ok(msg) = api.get_message(id).await {
                        results.push(msg);
                    }
                }
                self.search_results = results;
                self.status = format!("{} results for \"{}\"", ids.len(), query);
                self.mode = Mode::Normal;
            }
            Err(e) => {
                self.status = format!("Search error: {e}");
                self.mode = Mode::Normal;
            }
        }
    }

    /// Open $EDITOR with compose content, read back changes.
    pub fn open_editor(&mut self) -> std::io::Result<()> {
        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "nvim".to_string());
        let dir = std::env::temp_dir();
        let path = dir.join("boring-mail-compose.txt");

        // Write current compose state
        let content = format!(
            "To: {}\nCc: {}\nSubject: {}\n---\n{}",
            self.compose.to, self.compose.cc, self.compose.subject, self.compose.body_text()
        );
        std::fs::write(&path, &content)?;

        // Launch editor (terminal is already restored by caller)
        let status = std::process::Command::new(&editor)
            .arg(&path)
            .status()?;

        if status.success() {
            // Read back
            let edited = std::fs::read_to_string(&path)?;
            let mut in_headers = true;
            let mut body_lines = Vec::new();
            for line in edited.lines() {
                if in_headers {
                    if line == "---" {
                        in_headers = false;
                    } else if let Some(val) = line.strip_prefix("To: ") {
                        self.compose.to = val.to_string();
                    } else if let Some(val) = line.strip_prefix("Cc: ") {
                        self.compose.cc = val.to_string();
                    } else if let Some(val) = line.strip_prefix("Subject: ") {
                        self.compose.subject = val.to_string();
                    }
                } else {
                    body_lines.push(line.to_string());
                }
            }
            if !body_lines.is_empty() {
                self.compose.body = body_lines;
                self.compose.cursor_line = self.compose.body.len() - 1;
                self.compose.cursor_col = self.compose.body.last().map(|l| l.len()).unwrap_or(0);
            }
        }
        let _ = std::fs::remove_file(&path);
        Ok(())
    }
}
