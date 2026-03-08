use crate::db::connection::DbPool;
use crate::error::StorageError;
use crate::storage::models::*;
use crate::storage::DataStore;

#[derive(Clone)]
pub struct SqliteDataStore {
    db: DbPool,
}

impl SqliteDataStore {
    pub fn new(db: DbPool) -> Self {
        Self { db }
    }
}

impl DataStore for SqliteDataStore {
    async fn create_account(
        &self,
        name: &str,
        display_name: Option<&str>,
    ) -> Result<Account, StorageError> {
        let name = name.to_string();
        let display_name = display_name.map(|s| s.to_string());

        self.db
            .call(move |conn| {
                let id = uuid::Uuid::new_v4().to_string();
                let token = uuid::Uuid::new_v4().to_string();

                conn.execute(
                    "INSERT INTO accounts (id, name, display_name, bearer_token) VALUES (?1, ?2, ?3, ?4)",
                    rusqlite::params![id, name, display_name, token],
                )?;

                let account = conn.query_row(
                    "SELECT id, name, display_name, bearer_token, tmux_pane_id, active, created_at FROM accounts WHERE id = ?1",
                    rusqlite::params![id],
                    row_to_account,
                )?;

                Ok(account)
            })
            .await
            .map_err(StorageError::from)
    }

    async fn get_account_by_id(&self, id: &str) -> Result<Account, StorageError> {
        let id = id.to_string();
        self.db
            .call(move |conn| {
                conn.query_row(
                    "SELECT id, name, display_name, bearer_token, tmux_pane_id, active, created_at FROM accounts WHERE id = ?1",
                    rusqlite::params![id],
                    row_to_account,
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => {
                        tokio_rusqlite::Error::Other(Box::new(StorageError::NotFound(format!("account {id}"))))
                    }
                    e => tokio_rusqlite::Error::Rusqlite(e),
                })
            })
            .await
            .map_err(storage_err_from_tokio)
    }

    async fn get_account_by_name(&self, name: &str) -> Result<Account, StorageError> {
        let name = name.to_string();
        self.db
            .call(move |conn| {
                conn.query_row(
                    "SELECT id, name, display_name, bearer_token, tmux_pane_id, active, created_at FROM accounts WHERE name = ?1",
                    rusqlite::params![name],
                    row_to_account,
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => {
                        tokio_rusqlite::Error::Other(Box::new(StorageError::NotFound(format!("account name={name}"))))
                    }
                    e => tokio_rusqlite::Error::Rusqlite(e),
                })
            })
            .await
            .map_err(storage_err_from_tokio)
    }

    async fn get_account_by_token(&self, token: &str) -> Result<Account, StorageError> {
        let token = token.to_string();
        self.db
            .call(move |conn| {
                conn.query_row(
                    "SELECT id, name, display_name, bearer_token, tmux_pane_id, active, created_at FROM accounts WHERE bearer_token = ?1",
                    rusqlite::params![token],
                    row_to_account,
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => {
                        tokio_rusqlite::Error::Other(Box::new(StorageError::NotFound("invalid token".to_string())))
                    }
                    e => tokio_rusqlite::Error::Rusqlite(e),
                })
            })
            .await
            .map_err(storage_err_from_tokio)
    }

    async fn update_pane(
        &self,
        account_id: &str,
        pane_id: &str,
    ) -> Result<(), StorageError> {
        let account_id = account_id.to_string();
        let pane_id = pane_id.to_string();
        self.db
            .call(move |conn| {
                let rows = conn.execute(
                    "UPDATE accounts SET tmux_pane_id = ?1 WHERE id = ?2",
                    rusqlite::params![pane_id, account_id],
                )?;
                if rows == 0 {
                    return Err(tokio_rusqlite::Error::Other(Box::new(
                        StorageError::NotFound(format!("account {account_id}")),
                    )));
                }
                Ok(())
            })
            .await
            .map_err(storage_err_from_tokio)
    }

    async fn insert_message(&self, msg: NewMessage) -> Result<Message, StorageError> {
        self.db
            .call(move |conn| {
                let tx = conn.transaction()?;
                let msg_id = uuid::Uuid::new_v4().to_string();
                let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string();
                let snippet = make_snippet(&msg.body);
                let reply_requested = msg.reply_by.is_some();

                // Resolve or create thread
                let thread_id = if let Some(ref tid) = msg.thread_id {
                    // Verify thread exists
                    let exists: bool = tx.query_row(
                        "SELECT COUNT(*) > 0 FROM threads WHERE id = ?1",
                        rusqlite::params![tid],
                        |row| row.get(0),
                    )?;
                    if !exists {
                        return Err(tokio_rusqlite::Error::Other(Box::new(
                            StorageError::NotFound(format!("thread {tid}")),
                        )));
                    }
                    tid.clone()
                } else if let Some(ref reply_to) = msg.in_reply_to {
                    // Inherit thread from parent
                    tx.query_row(
                        "SELECT thread_id FROM messages WHERE id = ?1",
                        rusqlite::params![reply_to],
                        |row| row.get::<_, String>(0),
                    )
                    .map_err(|_| {
                        tokio_rusqlite::Error::Other(Box::new(StorageError::NotFound(
                            format!("in_reply_to message {reply_to}"),
                        )))
                    })?
                } else {
                    // New thread
                    let tid = uuid::Uuid::new_v4().to_string();
                    tx.execute(
                        "INSERT INTO threads (id, subject, snippet, last_message_at, message_count, participants) VALUES (?1, ?2, ?3, ?4, 0, '[]')",
                        rusqlite::params![tid, msg.subject, snippet, now],
                    )?;
                    tid
                };

                // Compress body if > 512 bytes
                let (stored_body, compressed) = if msg.body.len() > 512 {
                    let encoded = zstd::encode_all(msg.body.as_bytes(), 3)
                        .map_err(|e| tokio_rusqlite::Error::Other(Box::new(StorageError::BlobIo(e))))?;
                    // Use base64 to store compressed bytes in TEXT column
                    use base64::Engine;
                    let b64 = base64::engine::general_purpose::STANDARD.encode(&encoded);
                    (b64, true)
                } else {
                    (msg.body.clone(), false)
                };

                // Insert message
                tx.execute(
                    "INSERT INTO messages (id, thread_id, from_account, subject, body, snippet, internal_date, in_reply_to, reply_by, reply_requested, source, compressed)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                    rusqlite::params![
                        msg_id, thread_id, msg.from_account, msg.subject, stored_body,
                        snippet, now, msg.in_reply_to, msg.reply_by, reply_requested, msg.source, compressed,
                    ],
                )?;

                // Insert recipients
                for to in &msg.to {
                    tx.execute(
                        "INSERT INTO message_recipients (message_id, account_id, recipient_type) VALUES (?1, ?2, 'to')",
                        rusqlite::params![msg_id, to],
                    )?;
                }
                for cc in &msg.cc {
                    tx.execute(
                        "INSERT INTO message_recipients (message_id, account_id, recipient_type) VALUES (?1, ?2, 'cc')",
                        rusqlite::params![msg_id, cc],
                    )?;
                }

                // Assign labels: sender gets SENT, recipients get INBOX + UNREAD
                tx.execute(
                    "INSERT INTO message_labels (message_id, account_id, label) VALUES (?1, ?2, 'SENT')",
                    rusqlite::params![msg_id, msg.from_account],
                )?;

                let all_recipients: Vec<&str> = msg.to.iter().chain(msg.cc.iter()).map(|s| s.as_str()).collect();
                for recip in &all_recipients {
                    tx.execute(
                        "INSERT OR IGNORE INTO message_labels (message_id, account_id, label) VALUES (?1, ?2, 'INBOX')",
                        rusqlite::params![msg_id, recip],
                    )?;
                    tx.execute(
                        "INSERT OR IGNORE INTO message_labels (message_id, account_id, label) VALUES (?1, ?2, 'UNREAD')",
                        rusqlite::params![msg_id, recip],
                    )?;
                }

                // Custom labels from request
                for label in &msg.labels {
                    // Add to sender
                    tx.execute(
                        "INSERT OR IGNORE INTO message_labels (message_id, account_id, label) VALUES (?1, ?2, ?3)",
                        rusqlite::params![msg_id, msg.from_account, label],
                    )?;
                    // If ISSUE label, also add OPEN
                    if label == "ISSUE" {
                        tx.execute(
                            "INSERT OR IGNORE INTO message_labels (message_id, account_id, label) VALUES (?1, ?2, 'OPEN')",
                            rusqlite::params![msg_id, msg.from_account],
                        )?;
                    }
                }

                // Update thread
                let participants_json = {
                    let mut parts: Vec<String> = vec![msg.from_account.clone()];
                    for r in &all_recipients {
                        if !parts.contains(&r.to_string()) {
                            parts.push(r.to_string());
                        }
                    }
                    serde_json::to_string(&parts).unwrap_or_else(|_| "[]".to_string())
                };

                tx.execute(
                    "UPDATE threads SET snippet = ?1, last_message_at = ?2, message_count = message_count + 1, participants = ?3 WHERE id = ?4",
                    rusqlite::params![snippet, now, participants_json, thread_id],
                )?;

                // Index in FTS5
                tx.execute(
                    "INSERT INTO messages_fts (rowid, subject, body) SELECT rowid, subject, body FROM messages WHERE id = ?1",
                    rusqlite::params![msg_id],
                )?;

                // Attach blobs
                let has_attachments = !msg.attachments.is_empty();
                for blob_hash in &msg.attachments {
                    tx.execute(
                        "INSERT INTO attachments (message_id, blob_hash) VALUES (?1, ?2)",
                        rusqlite::params![msg_id, blob_hash],
                    )?;
                }
                if has_attachments {
                    tx.execute(
                        "UPDATE messages SET has_attachments = 1 WHERE id = ?1",
                        rusqlite::params![msg_id],
                    )?;
                }

                tx.commit()?;

                // Build return value
                let recipients: Vec<Recipient> = msg.to.iter().map(|id| Recipient {
                    account_id: id.clone(),
                    recipient_type: "to".to_string(),
                }).chain(msg.cc.iter().map(|id| Recipient {
                    account_id: id.clone(),
                    recipient_type: "cc".to_string(),
                })).collect();

                let mut labels = vec!["SENT".to_string()];
                labels.extend(msg.labels.clone());

                Ok(Message {
                    id: msg_id,
                    thread_id,
                    from_account: msg.from_account,
                    subject: msg.subject,
                    body: msg.body,
                    snippet: snippet.to_string(),
                    has_attachments: has_attachments,
                    internal_date: now,
                    in_reply_to: msg.in_reply_to,
                    reply_by: msg.reply_by,
                    reply_requested,
                    labels,
                    recipients,
                    source: msg.source,
                })
            })
            .await
            .map_err(storage_err_from_tokio)
    }

    async fn get_message(&self, id: &str) -> Result<Message, StorageError> {
        let id = id.to_string();
        self.db
            .call(move |conn| {
                let msg = conn.query_row(
                    "SELECT id, thread_id, from_account, subject, body, snippet, has_attachments, internal_date, in_reply_to, reply_by, reply_requested, source, compressed
                     FROM messages WHERE id = ?1",
                    rusqlite::params![id],
                    |row| {
                        let stored_body: String = row.get(4)?;
                        let compressed: bool = row.get::<_, i32>(12)? != 0;
                        Ok(Message {
                            id: row.get(0)?,
                            thread_id: row.get(1)?,
                            from_account: row.get(2)?,
                            subject: row.get(3)?,
                            body: decompress_body(&stored_body, compressed),
                            snippet: row.get(5)?,
                            has_attachments: row.get::<_, i32>(6)? != 0,
                            internal_date: row.get(7)?,
                            in_reply_to: row.get(8)?,
                            reply_by: row.get(9)?,
                            reply_requested: row.get::<_, i32>(10)? != 0,
                            labels: vec![],
                            recipients: vec![],
                            source: row.get(11)?,
                        })
                    },
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => {
                        tokio_rusqlite::Error::Other(Box::new(StorageError::NotFound(format!("message {id}"))))
                    }
                    e => tokio_rusqlite::Error::Rusqlite(e),
                })?;

                // Load labels (for the sender's perspective — caller should specify account_id for per-account labels)
                // For now, load all labels for this message
                let mut stmt = conn.prepare(
                    "SELECT DISTINCT label FROM message_labels WHERE message_id = ?1"
                )?;
                let labels: Vec<String> = stmt
                    .query_map(rusqlite::params![msg.id], |row| row.get(0))?
                    .filter_map(|r| r.ok())
                    .collect();

                // Load recipients
                let mut stmt = conn.prepare(
                    "SELECT account_id, recipient_type FROM message_recipients WHERE message_id = ?1"
                )?;
                let recipients: Vec<Recipient> = stmt
                    .query_map(rusqlite::params![msg.id], |row| {
                        Ok(Recipient {
                            account_id: row.get(0)?,
                            recipient_type: row.get(1)?,
                        })
                    })?
                    .filter_map(|r| r.ok())
                    .collect();

                Ok(Message { labels, recipients, ..msg })
            })
            .await
            .map_err(storage_err_from_tokio)
    }

    async fn list_messages(
        &self,
        account_id: &str,
        label: &str,
        max_results: u32,
        page_token: Option<&str>,
    ) -> Result<MessageList, StorageError> {
        let account_id = account_id.to_string();
        let label = label.to_string();
        let page_token = page_token.map(|s| s.to_string());

        self.db
            .call(move |conn| {
                let mut query = String::from(
                    "SELECT m.id, m.thread_id, m.from_account, m.subject, m.body, m.snippet, m.has_attachments, m.internal_date, m.in_reply_to, m.reply_by, m.reply_requested, m.source, m.compressed
                     FROM messages m
                     JOIN message_labels ml ON m.id = ml.message_id
                     WHERE ml.account_id = ?1 AND ml.label = ?2"
                );

                let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![
                    Box::new(account_id.clone()),
                    Box::new(label.clone()),
                ];

                if let Some(ref token) = page_token {
                    query.push_str(" AND m.internal_date < ?3");
                    params.push(Box::new(token.clone()));
                }

                query.push_str(" ORDER BY m.internal_date DESC LIMIT ?");
                let limit = max_results + 1; // fetch one extra for next_page_token
                params.push(Box::new(limit));

                let mut stmt = conn.prepare(&query)?;
                let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
                let mut messages: Vec<Message> = stmt
                    .query_map(param_refs.as_slice(), |row| {
                        let stored_body: String = row.get(4)?;
                        let compressed: bool = row.get::<_, i32>(12)? != 0;
                        Ok(Message {
                            id: row.get(0)?,
                            thread_id: row.get(1)?,
                            from_account: row.get(2)?,
                            subject: row.get(3)?,
                            body: decompress_body(&stored_body, compressed),
                            snippet: row.get(5)?,
                            has_attachments: row.get::<_, i32>(6)? != 0,
                            internal_date: row.get(7)?,
                            in_reply_to: row.get(8)?,
                            reply_by: row.get(9)?,
                            reply_requested: row.get::<_, i32>(10)? != 0,
                            labels: vec![],
                            recipients: vec![],
                            source: row.get(11)?,
                        })
                    })?
                    .filter_map(|r| r.ok())
                    .collect();

                let next_page_token = if messages.len() > max_results as usize {
                    let last = messages.pop().unwrap();
                    Some(last.internal_date)
                } else {
                    None
                };

                let result_size_estimate = messages.len() as u32;

                Ok(MessageList {
                    messages,
                    next_page_token,
                    result_size_estimate,
                })
            })
            .await
            .map_err(storage_err_from_tokio)
    }

    async fn delete_message(&self, id: &str) -> Result<(), StorageError> {
        let id = id.to_string();
        self.db
            .call(move |conn| {
                let tx = conn.transaction()?;
                // Delete FTS entry
                tx.execute(
                    "DELETE FROM messages_fts WHERE rowid = (SELECT rowid FROM messages WHERE id = ?1)",
                    rusqlite::params![id],
                )?;
                tx.execute("DELETE FROM message_labels WHERE message_id = ?1", rusqlite::params![id])?;
                tx.execute("DELETE FROM message_recipients WHERE message_id = ?1", rusqlite::params![id])?;
                tx.execute("DELETE FROM attachments WHERE message_id = ?1", rusqlite::params![id])?;
                let rows = tx.execute("DELETE FROM messages WHERE id = ?1", rusqlite::params![id])?;
                if rows == 0 {
                    return Err(tokio_rusqlite::Error::Other(Box::new(
                        StorageError::NotFound(format!("message {id}")),
                    )));
                }
                tx.commit()?;
                Ok(())
            })
            .await
            .map_err(storage_err_from_tokio)
    }

    async fn add_labels(
        &self,
        message_id: &str,
        account_id: &str,
        labels: &[String],
    ) -> Result<(), StorageError> {
        let message_id = message_id.to_string();
        let account_id = account_id.to_string();
        let labels = labels.to_vec();

        self.db
            .call(move |conn| {
                for label in &labels {
                    conn.execute(
                        "INSERT OR IGNORE INTO message_labels (message_id, account_id, label) VALUES (?1, ?2, ?3)",
                        rusqlite::params![message_id, account_id, label],
                    )?;
                }
                Ok(())
            })
            .await
            .map_err(StorageError::from)
    }

    async fn remove_labels(
        &self,
        message_id: &str,
        account_id: &str,
        labels: &[String],
    ) -> Result<(), StorageError> {
        let message_id = message_id.to_string();
        let account_id = account_id.to_string();
        let labels = labels.to_vec();

        self.db
            .call(move |conn| {
                for label in &labels {
                    conn.execute(
                        "DELETE FROM message_labels WHERE message_id = ?1 AND account_id = ?2 AND label = ?3",
                        rusqlite::params![message_id, account_id, label],
                    )?;
                }
                Ok(())
            })
            .await
            .map_err(StorageError::from)
    }

    async fn get_labels(
        &self,
        message_id: &str,
        account_id: &str,
    ) -> Result<Vec<String>, StorageError> {
        let message_id = message_id.to_string();
        let account_id = account_id.to_string();

        self.db
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT label FROM message_labels WHERE message_id = ?1 AND account_id = ?2"
                )?;
                let labels: Vec<String> = stmt
                    .query_map(rusqlite::params![message_id, account_id], |row| row.get(0))?
                    .filter_map(|r| r.ok())
                    .collect();
                Ok(labels)
            })
            .await
            .map_err(StorageError::from)
    }

    async fn list_labels_with_counts(
        &self,
        account_id: &str,
    ) -> Result<Vec<LabelCount>, StorageError> {
        let account_id = account_id.to_string();

        self.db
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT ml.label,
                            COALESCE(l.label_type, 'user') as label_type,
                            COUNT(*) as message_count,
                            SUM(CASE WHEN EXISTS(
                                SELECT 1 FROM message_labels ul
                                WHERE ul.message_id = ml.message_id
                                AND ul.account_id = ml.account_id
                                AND ul.label = 'UNREAD'
                            ) THEN 1 ELSE 0 END) as unread_count
                     FROM message_labels ml
                     LEFT JOIN labels l ON l.name = ml.label AND (l.account_id IS NULL OR l.account_id = ml.account_id)
                     WHERE ml.account_id = ?1
                     GROUP BY ml.label
                     ORDER BY ml.label"
                )?;

                let labels: Vec<LabelCount> = stmt
                    .query_map(rusqlite::params![account_id], |row| {
                        Ok(LabelCount {
                            name: row.get(0)?,
                            label_type: row.get(1)?,
                            message_count: row.get(2)?,
                            unread_count: row.get(3)?,
                        })
                    })?
                    .filter_map(|r| r.ok())
                    .collect();

                Ok(labels)
            })
            .await
            .map_err(StorageError::from)
    }

    async fn attach_blob(
        &self,
        message_id: &str,
        blob_hash: &str,
        filename: &str,
        content_type: &str,
        size: u64,
    ) -> Result<(), StorageError> {
        let message_id = message_id.to_string();
        let blob_hash = blob_hash.to_string();
        let filename = filename.to_string();
        let content_type = content_type.to_string();

        self.db
            .call(move |conn| {
                conn.execute(
                    "INSERT INTO attachments (message_id, blob_hash, filename, content_type, size) VALUES (?1, ?2, ?3, ?4, ?5)",
                    rusqlite::params![message_id, blob_hash, filename, content_type, size],
                )?;
                // Mark message as having attachments
                conn.execute(
                    "UPDATE messages SET has_attachments = 1 WHERE id = ?1",
                    rusqlite::params![message_id],
                )?;
                Ok(())
            })
            .await
            .map_err(StorageError::from)
    }

    async fn get_attachments(&self, message_id: &str) -> Result<Vec<Attachment>, StorageError> {
        let message_id = message_id.to_string();
        self.db
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT blob_hash, filename, content_type, size FROM attachments WHERE message_id = ?1",
                )?;
                let attachments = stmt
                    .query_map(rusqlite::params![message_id], |row| {
                        Ok(Attachment {
                            blob_hash: row.get(0)?,
                            filename: row.get(1)?,
                            content_type: row.get(2)?,
                            size: row.get(3)?,
                        })
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(attachments)
            })
            .await
            .map_err(StorageError::from)
    }

    async fn create_label(
        &self,
        account_id: &str,
        name: &str,
    ) -> Result<(String, String), StorageError> {
        let account_id = account_id.to_string();
        let name = name.to_string();

        self.db
            .call(move |conn| {
                let id = uuid::Uuid::new_v4().to_string();
                conn.execute(
                    "INSERT INTO labels (id, account_id, name, label_type) VALUES (?1, ?2, ?3, 'user')",
                    rusqlite::params![id, account_id, name],
                )?;
                Ok((id, name))
            })
            .await
            .map_err(StorageError::from)
    }

    async fn delete_label(
        &self,
        account_id: &str,
        name: &str,
    ) -> Result<(), StorageError> {
        let account_id = account_id.to_string();
        let name = name.to_string();

        self.db
            .call(move |conn| {
                // Only delete user labels, not system labels
                let rows = conn.execute(
                    "DELETE FROM labels WHERE account_id = ?1 AND name = ?2 AND label_type = 'user'",
                    rusqlite::params![account_id, name],
                )?;
                if rows == 0 {
                    return Err(tokio_rusqlite::Error::Other(Box::new(
                        StorageError::NotFound(format!("label {name}")),
                    )));
                }
                // Also remove from message_labels
                conn.execute(
                    "DELETE FROM message_labels WHERE account_id = ?1 AND label = ?2",
                    rusqlite::params![account_id, name],
                )?;
                Ok(())
            })
            .await
            .map_err(storage_err_from_tokio)
    }

    async fn batch_modify_labels(
        &self,
        message_ids: &[String],
        account_id: &str,
        add: &[String],
        remove: &[String],
    ) -> Result<(), StorageError> {
        let message_ids = message_ids.to_vec();
        let account_id = account_id.to_string();
        let add = add.to_vec();
        let remove = remove.to_vec();

        self.db
            .call(move |conn| {
                let tx = conn.transaction()?;
                for msg_id in &message_ids {
                    for label in &add {
                        tx.execute(
                            "INSERT OR IGNORE INTO message_labels (message_id, account_id, label) VALUES (?1, ?2, ?3)",
                            rusqlite::params![msg_id, account_id, label],
                        )?;
                    }
                    for label in &remove {
                        tx.execute(
                            "DELETE FROM message_labels WHERE message_id = ?1 AND account_id = ?2 AND label = ?3",
                            rusqlite::params![msg_id, account_id, label],
                        )?;
                    }
                }
                tx.commit()?;
                Ok(())
            })
            .await
            .map_err(StorageError::from)
    }

    async fn create_list(
        &self,
        name: &str,
        description: &str,
    ) -> Result<String, StorageError> {
        let name = name.to_string();
        let description = description.to_string();

        self.db
            .call(move |conn| {
                let id = uuid::Uuid::new_v4().to_string();
                conn.execute(
                    "INSERT INTO lists (id, name, description) VALUES (?1, ?2, ?3)",
                    rusqlite::params![id, name, description],
                )?;
                Ok(id)
            })
            .await
            .map_err(StorageError::from)
    }

    async fn subscribe_to_list(
        &self,
        list_id: &str,
        account_id: &str,
    ) -> Result<(), StorageError> {
        let list_id = list_id.to_string();
        let account_id = account_id.to_string();

        self.db
            .call(move |conn| {
                conn.execute(
                    "INSERT OR IGNORE INTO list_members (list_id, account_id) VALUES (?1, ?2)",
                    rusqlite::params![list_id, account_id],
                )?;
                Ok(())
            })
            .await
            .map_err(StorageError::from)
    }

    async fn unsubscribe_from_list(
        &self,
        list_id: &str,
        account_id: &str,
    ) -> Result<(), StorageError> {
        let list_id = list_id.to_string();
        let account_id = account_id.to_string();

        self.db
            .call(move |conn| {
                conn.execute(
                    "DELETE FROM list_members WHERE list_id = ?1 AND account_id = ?2",
                    rusqlite::params![list_id, account_id],
                )?;
                Ok(())
            })
            .await
            .map_err(StorageError::from)
    }

    async fn get_list_members(
        &self,
        list_id: &str,
    ) -> Result<Vec<String>, StorageError> {
        let list_id = list_id.to_string();

        self.db
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT account_id FROM list_members WHERE list_id = ?1"
                )?;
                let members: Vec<String> = stmt
                    .query_map(rusqlite::params![list_id], |row| row.get(0))?
                    .filter_map(|r| r.ok())
                    .collect();
                Ok(members)
            })
            .await
            .map_err(StorageError::from)
    }

    async fn get_list_by_name(
        &self,
        name: &str,
    ) -> Result<(String, String, String), StorageError> {
        let name = name.to_string();

        self.db
            .call(move |conn| {
                conn.query_row(
                    "SELECT id, name, description FROM lists WHERE name = ?1",
                    rusqlite::params![name],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => {
                        tokio_rusqlite::Error::Other(Box::new(StorageError::NotFound(format!("list {name}"))))
                    }
                    e => tokio_rusqlite::Error::Rusqlite(e),
                })
            })
            .await
            .map_err(storage_err_from_tokio)
    }

    async fn get_thread(&self, id: &str) -> Result<Thread, StorageError> {
        let id = id.to_string();
        self.db
            .call(move |conn| {
                let thread = conn.query_row(
                    "SELECT id, subject, snippet, last_message_at, message_count, participants FROM threads WHERE id = ?1",
                    rusqlite::params![id],
                    |row| {
                        let participants_str: String = row.get(5)?;
                        let participants: Vec<String> = serde_json::from_str(&participants_str).unwrap_or_default();
                        Ok(Thread {
                            id: row.get(0)?,
                            subject: row.get(1)?,
                            snippet: row.get(2)?,
                            last_message_at: row.get(3)?,
                            message_count: row.get(4)?,
                            participants,
                            messages: vec![],
                        })
                    },
                ).map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => {
                        tokio_rusqlite::Error::Other(Box::new(StorageError::NotFound(format!("thread {id}"))))
                    }
                    e => tokio_rusqlite::Error::Rusqlite(e),
                })?;

                // Load messages in thread
                let mut stmt = conn.prepare(
                    "SELECT id, thread_id, from_account, subject, body, snippet, has_attachments, internal_date, in_reply_to, reply_by, reply_requested, source, compressed
                     FROM messages WHERE thread_id = ?1 ORDER BY internal_date ASC"
                )?;
                let messages: Vec<Message> = stmt
                    .query_map(rusqlite::params![thread.id], |row| {
                        let stored_body: String = row.get(4)?;
                        let compressed: bool = row.get::<_, i32>(12)? != 0;
                        Ok(Message {
                            id: row.get(0)?,
                            thread_id: row.get(1)?,
                            from_account: row.get(2)?,
                            subject: row.get(3)?,
                            body: decompress_body(&stored_body, compressed),
                            snippet: row.get(5)?,
                            has_attachments: row.get::<_, i32>(6)? != 0,
                            internal_date: row.get(7)?,
                            in_reply_to: row.get(8)?,
                            reply_by: row.get(9)?,
                            reply_requested: row.get::<_, i32>(10)? != 0,
                            labels: vec![],
                            recipients: vec![],
                            source: row.get(11)?,
                        })
                    })?
                    .filter_map(|r| r.ok())
                    .collect();

                Ok(Thread { messages, ..thread })
            })
            .await
            .map_err(storage_err_from_tokio)
    }

    async fn list_threads(
        &self,
        account_id: &str,
        label: &str,
        max_results: u32,
        page_token: Option<&str>,
    ) -> Result<ThreadList, StorageError> {
        let account_id = account_id.to_string();
        let label = label.to_string();
        let page_token = page_token.map(|s| s.to_string());

        self.db
            .call(move |conn| {
                let mut query = String::from(
                    "SELECT DISTINCT t.id, t.subject, t.snippet, t.last_message_at, t.message_count, t.participants
                     FROM threads t
                     JOIN messages m ON m.thread_id = t.id
                     JOIN message_labels ml ON ml.message_id = m.id
                     WHERE ml.account_id = ?1 AND ml.label = ?2"
                );

                let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![
                    Box::new(account_id),
                    Box::new(label),
                ];

                if let Some(ref token) = page_token {
                    query.push_str(" AND t.last_message_at < ?3");
                    params.push(Box::new(token.clone()));
                }

                query.push_str(" ORDER BY t.last_message_at DESC LIMIT ?");
                let limit = max_results + 1;
                params.push(Box::new(limit));

                let mut stmt = conn.prepare(&query)?;
                let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
                let mut threads: Vec<Thread> = stmt
                    .query_map(param_refs.as_slice(), |row| {
                        let participants_str: String = row.get(5)?;
                        let participants: Vec<String> = serde_json::from_str(&participants_str).unwrap_or_default();
                        Ok(Thread {
                            id: row.get(0)?,
                            subject: row.get(1)?,
                            snippet: row.get(2)?,
                            last_message_at: row.get(3)?,
                            message_count: row.get(4)?,
                            participants,
                            messages: vec![],
                        })
                    })?
                    .filter_map(|r| r.ok())
                    .collect();

                let next_page_token = if threads.len() > max_results as usize {
                    let last = threads.pop().unwrap();
                    Some(last.last_message_at)
                } else {
                    None
                };

                let result_size_estimate = threads.len() as u32;

                Ok(ThreadList {
                    threads,
                    next_page_token,
                    result_size_estimate,
                })
            })
            .await
            .map_err(storage_err_from_tokio)
    }

    async fn get_unread_count(&self, account_id: &str) -> Result<u32, StorageError> {
        let account_id = account_id.to_string();
        self.db
            .call(move |conn| {
                conn.query_row(
                    "SELECT COUNT(*) FROM message_labels WHERE account_id = ?1 AND label = 'UNREAD'",
                    rusqlite::params![account_id],
                    |row| row.get(0),
                )
                .map_err(tokio_rusqlite::Error::Rusqlite)
            })
            .await
            .map_err(StorageError::from)
    }

    async fn get_pending_replies(&self, account_id: &str) -> Result<Vec<PendingReply>, StorageError> {
        let account_id = account_id.to_string();
        self.db
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT m.id, m.from_account, m.subject, m.reply_by, m.internal_date
                     FROM messages m
                     JOIN message_recipients mr ON m.id = mr.message_id
                     WHERE mr.account_id = ?1 AND m.reply_requested = 1
                     AND NOT EXISTS (
                         SELECT 1 FROM messages reply
                         WHERE reply.in_reply_to = m.id AND reply.from_account = ?1
                     )
                     ORDER BY m.reply_by ASC NULLS LAST"
                )?;
                let replies: Vec<PendingReply> = stmt
                    .query_map(rusqlite::params![account_id], |row| {
                        Ok(PendingReply {
                            message_id: row.get(0)?,
                            from_account: row.get(1)?,
                            subject: row.get(2)?,
                            reply_by: row.get(3)?,
                            sent_at: row.get(4)?,
                        })
                    })?
                    .filter_map(|r| r.ok())
                    .collect();
                Ok(replies)
            })
            .await
            .map_err(StorageError::from)
    }

    async fn label_overdue_messages(&self) -> Result<u32, StorageError> {
        self.db
            .call(move |conn| {
                let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
                // Find messages with reply_by in the past that don't have OVERDUE label yet
                let mut stmt = conn.prepare(
                    "SELECT DISTINCT m.id, mr.account_id
                     FROM messages m
                     JOIN message_recipients mr ON m.id = mr.message_id
                     WHERE m.reply_requested = 1
                     AND m.reply_by < ?1
                     AND NOT EXISTS (
                         SELECT 1 FROM messages reply
                         WHERE reply.in_reply_to = m.id AND reply.from_account = mr.account_id
                     )
                     AND NOT EXISTS (
                         SELECT 1 FROM message_labels ml
                         WHERE ml.message_id = m.id AND ml.account_id = mr.account_id AND ml.label = 'OVERDUE'
                     )"
                )?;

                let overdue: Vec<(String, String)> = stmt
                    .query_map(rusqlite::params![now], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                    })?
                    .filter_map(|r| r.ok())
                    .collect();

                let count = overdue.len() as u32;
                for (msg_id, account_id) in &overdue {
                    conn.execute(
                        "INSERT OR IGNORE INTO message_labels (message_id, account_id, label) VALUES (?1, ?2, 'OVERDUE')",
                        rusqlite::params![msg_id, account_id],
                    )?;
                }

                Ok(count)
            })
            .await
            .map_err(StorageError::from)
    }
}

fn row_to_account(row: &rusqlite::Row) -> rusqlite::Result<Account> {
    Ok(Account {
        id: row.get(0)?,
        name: row.get(1)?,
        display_name: row.get(2)?,
        bearer_token: row.get(3)?,
        tmux_pane_id: row.get(4)?,
        active: row.get::<_, i32>(5)? != 0,
        created_at: row.get(6)?,
    })
}

fn decompress_body(stored_body: &str, compressed: bool) -> String {
    if !compressed {
        return stored_body.to_string();
    }
    use base64::Engine;
    let bytes = match base64::engine::general_purpose::STANDARD.decode(stored_body) {
        Ok(b) => b,
        Err(_) => return stored_body.to_string(),
    };
    match zstd::decode_all(bytes.as_slice()) {
        Ok(decoded) => String::from_utf8(decoded).unwrap_or_else(|_| stored_body.to_string()),
        Err(_) => stored_body.to_string(),
    }
}

fn make_snippet(body: &str) -> String {
    let s: String = body.chars().take(200).collect();
    if body.len() > 200 {
        format!("{s}...")
    } else {
        s
    }
}

fn storage_err_from_tokio(e: tokio_rusqlite::Error) -> StorageError {
    match e {
        tokio_rusqlite::Error::Other(inner) => {
            if let Some(se) = inner.downcast_ref::<StorageError>() {
                match se {
                    StorageError::NotFound(msg) => StorageError::NotFound(msg.clone()),
                    _ => StorageError::Database(tokio_rusqlite::Error::Other(inner)),
                }
            } else {
                StorageError::Database(tokio_rusqlite::Error::Other(inner))
            }
        }
        e => StorageError::Database(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema::init_schema;

    async fn test_store() -> SqliteDataStore {
        let conn = tokio_rusqlite::Connection::open_in_memory().await.unwrap();
        conn.call(|c| { init_schema(c).unwrap(); Ok(()) }).await.unwrap();
        SqliteDataStore::new(conn)
    }

    #[tokio::test]
    async fn test_create_and_get_account() {
        let store = test_store().await;

        let account = store.create_account("agent-1", Some("Agent One")).await.unwrap();
        assert_eq!(account.name, "agent-1");
        assert_eq!(account.display_name, Some("Agent One".to_string()));
        assert!(account.active);
        assert!(!account.bearer_token.is_empty());

        // Get by ID
        let fetched = store.get_account_by_id(&account.id).await.unwrap();
        assert_eq!(fetched.name, "agent-1");

        // Get by name
        let fetched = store.get_account_by_name("agent-1").await.unwrap();
        assert_eq!(fetched.id, account.id);

        // Get by token
        let fetched = store.get_account_by_token(&account.bearer_token).await.unwrap();
        assert_eq!(fetched.id, account.id);
    }

    #[tokio::test]
    async fn test_account_not_found() {
        let store = test_store().await;
        let result = store.get_account_by_id("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_duplicate_account_name() {
        let store = test_store().await;
        store.create_account("agent-1", None).await.unwrap();
        let result = store.create_account("agent-1", None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_update_pane() {
        let store = test_store().await;
        let account = store.create_account("agent-1", None).await.unwrap();
        assert!(account.tmux_pane_id.is_none());

        store.update_pane(&account.id, "%42").await.unwrap();
        let fetched = store.get_account_by_id(&account.id).await.unwrap();
        assert_eq!(fetched.tmux_pane_id, Some("%42".to_string()));
    }

    #[tokio::test]
    async fn test_send_and_get_message() {
        let store = test_store().await;
        let sender = store.create_account("sender", None).await.unwrap();
        let recipient = store.create_account("recipient", None).await.unwrap();

        let msg = NewMessage {
            from_account: sender.id.clone(),
            to: vec![recipient.id.clone()],
            cc: vec![],
            subject: "Hello".to_string(),
            body: "World".to_string(),
            thread_id: None,
            in_reply_to: None,
            reply_by: None,
            labels: vec![],
            source: None, attachments: vec![],
        };

        let sent = store.insert_message(msg).await.unwrap();
        assert_eq!(sent.subject, "Hello");
        assert_eq!(sent.from_account, sender.id);
        assert!(!sent.thread_id.is_empty());

        // Get the message back
        let fetched = store.get_message(&sent.id).await.unwrap();
        assert_eq!(fetched.subject, "Hello");
        assert_eq!(fetched.body, "World");
        assert!(fetched.recipients.iter().any(|r| r.account_id == recipient.id));
    }

    #[tokio::test]
    async fn test_message_labels_auto_assignment() {
        let store = test_store().await;
        let sender = store.create_account("sender", None).await.unwrap();
        let recipient = store.create_account("recipient", None).await.unwrap();

        let msg = NewMessage {
            from_account: sender.id.clone(),
            to: vec![recipient.id.clone()],
            cc: vec![],
            subject: "Test".to_string(),
            body: "Body".to_string(),
            thread_id: None,
            in_reply_to: None,
            reply_by: None,
            labels: vec![],
            source: None, attachments: vec![],
        };

        let sent = store.insert_message(msg).await.unwrap();

        // Sender should have SENT
        let sender_labels = store.get_labels(&sent.id, &sender.id).await.unwrap();
        assert!(sender_labels.contains(&"SENT".to_string()));

        // Recipient should have INBOX + UNREAD
        let recip_labels = store.get_labels(&sent.id, &recipient.id).await.unwrap();
        assert!(recip_labels.contains(&"INBOX".to_string()));
        assert!(recip_labels.contains(&"UNREAD".to_string()));
    }

    #[tokio::test]
    async fn test_list_messages_by_label() {
        let store = test_store().await;
        let sender = store.create_account("sender", None).await.unwrap();
        let recipient = store.create_account("recipient", None).await.unwrap();

        // Send 3 messages
        for i in 0..3 {
            let msg = NewMessage {
                from_account: sender.id.clone(),
                to: vec![recipient.id.clone()],
                cc: vec![],
                subject: format!("Message {i}"),
                body: format!("Body {i}"),
                thread_id: None,
                in_reply_to: None,
                reply_by: None,
                labels: vec![],
                source: None, attachments: vec![],
            };
            store.insert_message(msg).await.unwrap();
        }

        // Recipient inbox should have 3
        let list = store.list_messages(&recipient.id, "INBOX", 10, None).await.unwrap();
        assert_eq!(list.messages.len(), 3);

        // Sender sent should have 3
        let list = store.list_messages(&sender.id, "SENT", 10, None).await.unwrap();
        assert_eq!(list.messages.len(), 3);
    }

    #[tokio::test]
    async fn test_modify_labels() {
        let store = test_store().await;
        let sender = store.create_account("sender", None).await.unwrap();
        let recipient = store.create_account("recipient", None).await.unwrap();

        let msg = NewMessage {
            from_account: sender.id.clone(),
            to: vec![recipient.id.clone()],
            cc: vec![],
            subject: "Test".to_string(),
            body: "Body".to_string(),
            thread_id: None,
            in_reply_to: None,
            reply_by: None,
            labels: vec![],
            source: None, attachments: vec![],
        };
        let sent = store.insert_message(msg).await.unwrap();

        // Remove UNREAD, add STARRED
        store.remove_labels(&sent.id, &recipient.id, &["UNREAD".to_string()]).await.unwrap();
        store.add_labels(&sent.id, &recipient.id, &["STARRED".to_string()]).await.unwrap();

        let labels = store.get_labels(&sent.id, &recipient.id).await.unwrap();
        assert!(!labels.contains(&"UNREAD".to_string()));
        assert!(labels.contains(&"STARRED".to_string()));
        assert!(labels.contains(&"INBOX".to_string()));
    }

    #[tokio::test]
    async fn test_delete_message() {
        let store = test_store().await;
        let sender = store.create_account("sender", None).await.unwrap();
        let recipient = store.create_account("recipient", None).await.unwrap();

        let msg = NewMessage {
            from_account: sender.id.clone(),
            to: vec![recipient.id.clone()],
            cc: vec![],
            subject: "Delete me".to_string(),
            body: "Gone".to_string(),
            thread_id: None,
            in_reply_to: None,
            reply_by: None,
            labels: vec![],
            source: None, attachments: vec![],
        };
        let sent = store.insert_message(msg).await.unwrap();

        store.delete_message(&sent.id).await.unwrap();
        let result = store.get_message(&sent.id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_unread_count() {
        let store = test_store().await;
        let sender = store.create_account("sender", None).await.unwrap();
        let recipient = store.create_account("recipient", None).await.unwrap();

        // Send 2 messages
        for i in 0..2 {
            let msg = NewMessage {
                from_account: sender.id.clone(),
                to: vec![recipient.id.clone()],
                cc: vec![],
                subject: format!("Msg {i}"),
                body: "Body".to_string(),
                thread_id: None,
                in_reply_to: None,
                reply_by: None,
                labels: vec![],
                source: None, attachments: vec![],
            };
            store.insert_message(msg).await.unwrap();
        }

        let count = store.get_unread_count(&recipient.id).await.unwrap();
        assert_eq!(count, 2);

        // Sender should have 0 unread
        let count = store.get_unread_count(&sender.id).await.unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_thread_creation_and_get() {
        let store = test_store().await;
        let sender = store.create_account("sender", None).await.unwrap();
        let recipient = store.create_account("recipient", None).await.unwrap();

        let msg = NewMessage {
            from_account: sender.id.clone(),
            to: vec![recipient.id.clone()],
            cc: vec![],
            subject: "Thread test".to_string(),
            body: "First message".to_string(),
            thread_id: None,
            in_reply_to: None,
            reply_by: None,
            labels: vec![],
            source: None, attachments: vec![],
        };

        let sent = store.insert_message(msg).await.unwrap();

        // Reply
        let reply = NewMessage {
            from_account: recipient.id.clone(),
            to: vec![sender.id.clone()],
            cc: vec![],
            subject: "Re: Thread test".to_string(),
            body: "Reply message".to_string(),
            thread_id: None,
            in_reply_to: Some(sent.id.clone()),
            reply_by: None,
            labels: vec![],
            source: None, attachments: vec![],
        };
        let reply_sent = store.insert_message(reply).await.unwrap();
        assert_eq!(reply_sent.thread_id, sent.thread_id);

        // Get thread
        let thread = store.get_thread(&sent.thread_id).await.unwrap();
        assert_eq!(thread.message_count, 2);
        assert_eq!(thread.messages.len(), 2);
        assert_eq!(thread.subject, "Thread test");
    }

    #[tokio::test]
    async fn test_pending_replies() {
        let store = test_store().await;
        let sender = store.create_account("sender", None).await.unwrap();
        let recipient = store.create_account("recipient", None).await.unwrap();

        let msg = NewMessage {
            from_account: sender.id.clone(),
            to: vec![recipient.id.clone()],
            cc: vec![],
            subject: "Need reply".to_string(),
            body: "Please respond".to_string(),
            thread_id: None,
            in_reply_to: None,
            reply_by: Some("2026-03-09T00:00:00Z".to_string()),
            labels: vec![],
            source: None, attachments: vec![],
        };
        store.insert_message(msg).await.unwrap();

        let pending = store.get_pending_replies(&recipient.id).await.unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].subject, "Need reply");
    }

    #[tokio::test]
    async fn test_body_compression() {
        let store = test_store().await;
        let sender = store.create_account("sender", None).await.unwrap();
        let recipient = store.create_account("recipient", None).await.unwrap();

        // Create a body > 512 bytes that should be compressed
        let large_body = "x".repeat(1000);
        let msg = NewMessage {
            from_account: sender.id.clone(),
            to: vec![recipient.id.clone()],
            cc: vec![],
            subject: "Large body".to_string(),
            body: large_body.clone(),
            thread_id: None,
            in_reply_to: None,
            reply_by: None,
            labels: vec![],
            source: None, attachments: vec![],
        };

        let sent = store.insert_message(msg).await.unwrap();

        // Retrieve and verify body is decompressed correctly
        let fetched = store.get_message(&sent.id).await.unwrap();
        assert_eq!(fetched.body, large_body);
        assert_eq!(fetched.body.len(), 1000);

        // Verify the stored body in DB is actually compressed (smaller than original)
        let db = store.db.clone();
        let msg_id = sent.id.clone();
        let (stored_body, compressed): (String, bool) = db
            .call(move |conn| {
                conn.query_row(
                    "SELECT body, compressed FROM messages WHERE id = ?1",
                    rusqlite::params![msg_id],
                    |row| Ok((row.get(0)?, row.get::<_, i32>(1)? != 0)),
                )
                .map_err(|e| tokio_rusqlite::Error::Rusqlite(e))
            })
            .await
            .unwrap();
        assert!(compressed, "body should be marked as compressed");
        assert!(stored_body.len() < large_body.len(), "stored body should be smaller than original");
    }

    #[tokio::test]
    async fn test_label_overdue_messages() {
        let store = test_store().await;
        let sender = store.create_account("sender", None).await.unwrap();
        let recipient = store.create_account("recipient", None).await.unwrap();

        // Message with past deadline
        let msg = NewMessage {
            from_account: sender.id.clone(),
            to: vec![recipient.id.clone()],
            subject: "Overdue task".to_string(),
            body: "Needs reply".to_string(),
            reply_by: Some("2020-01-01T00:00:00Z".to_string()),
            ..Default::default()
        };
        let sent = store.insert_message(msg).await.unwrap();

        // Message with future deadline (should NOT be labeled)
        let msg2 = NewMessage {
            from_account: sender.id.clone(),
            to: vec![recipient.id.clone()],
            subject: "Future task".to_string(),
            body: "Not urgent".to_string(),
            reply_by: Some("2099-01-01T00:00:00Z".to_string()),
            ..Default::default()
        };
        store.insert_message(msg2).await.unwrap();

        // Run overdue check
        let labeled = store.label_overdue_messages().await.unwrap();
        assert_eq!(labeled, 1);

        // Verify OVERDUE label was added
        let labels = store.get_labels(&sent.id, &recipient.id).await.unwrap();
        assert!(labels.contains(&"OVERDUE".to_string()));

        // Running again should find 0 (already labeled)
        let labeled2 = store.label_overdue_messages().await.unwrap();
        assert_eq!(labeled2, 0);
    }
}
