use crate::db::connection::DbPool;
use crate::error::StorageError;
use crate::storage::models::*;
use crate::storage::DataStore;
use sqlx::Row;

#[derive(Clone)]
pub struct DoltDataStore {
    db: DbPool,
}

impl DoltDataStore {
    pub fn new(db: DbPool) -> Self {
        Self { db }
    }
}

impl DataStore for DoltDataStore {
    async fn create_account(
        &self,
        name: &str,
        display_name: Option<&str>,
        bio: Option<&str>,
    ) -> Result<Account, StorageError> {
        let id = uuid::Uuid::new_v4().to_string();
        let token = uuid::Uuid::new_v4().to_string();

        sqlx::query(
            "INSERT INTO accounts (id, name, display_name, bio, bearer_token) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(name)
        .bind(display_name)
        .bind(bio)
        .bind(&token)
        .execute(&self.db)
        .await?;

        let row = sqlx::query(
            "SELECT id, name, display_name, bio, bearer_token, tmux_pane_id, active, created_at FROM accounts WHERE id = ?",
        )
        .bind(&id)
        .fetch_one(&self.db)
        .await?;

        Ok(row_to_account(&row))
    }

    async fn get_account_by_id(&self, id: &str) -> Result<Account, StorageError> {
        let row = sqlx::query(
            "SELECT id, name, display_name, bio, bearer_token, tmux_pane_id, active, created_at FROM accounts WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.db)
        .await?;

        match row {
            Some(row) => Ok(row_to_account(&row)),
            None => Err(StorageError::NotFound(format!("account {id}"))),
        }
    }

    async fn get_account_by_name(&self, name: &str) -> Result<Account, StorageError> {
        let row = sqlx::query(
            "SELECT id, name, display_name, bio, bearer_token, tmux_pane_id, active, created_at FROM accounts WHERE name = ?",
        )
        .bind(name)
        .fetch_optional(&self.db)
        .await?;

        match row {
            Some(row) => Ok(row_to_account(&row)),
            None => Err(StorageError::NotFound(format!("account name={name}"))),
        }
    }

    async fn get_account_by_token(&self, token: &str) -> Result<Account, StorageError> {
        let row = sqlx::query(
            "SELECT id, name, display_name, bio, bearer_token, tmux_pane_id, active, created_at FROM accounts WHERE bearer_token = ?",
        )
        .bind(token)
        .fetch_optional(&self.db)
        .await?;

        match row {
            Some(row) => Ok(row_to_account(&row)),
            None => Err(StorageError::NotFound("invalid token".to_string())),
        }
    }

    async fn list_accounts(&self) -> Result<Vec<Account>, StorageError> {
        let rows = sqlx::query(
            "SELECT id, name, display_name, bio, bearer_token, tmux_pane_id, active, created_at FROM accounts ORDER BY created_at",
        )
        .fetch_all(&self.db)
        .await?;

        Ok(rows.iter().map(row_to_account).collect())
    }

    async fn update_profile(
        &self,
        account_id: &str,
        display_name: Option<&str>,
        bio: Option<&str>,
    ) -> Result<Account, StorageError> {
        let result = sqlx::query(
            "UPDATE accounts SET display_name = COALESCE(?, display_name), bio = COALESCE(?, bio) WHERE id = ?",
        )
        .bind(display_name)
        .bind(bio)
        .bind(account_id)
        .execute(&self.db)
        .await?;

        if result.rows_affected() == 0 {
            return Err(StorageError::NotFound(format!("account {account_id}")));
        }

        let row = sqlx::query(
            "SELECT id, name, display_name, bio, bearer_token, tmux_pane_id, active, created_at FROM accounts WHERE id = ?",
        )
        .bind(account_id)
        .fetch_one(&self.db)
        .await?;

        Ok(row_to_account(&row))
    }

    async fn update_pane(
        &self,
        account_id: &str,
        pane_id: &str,
    ) -> Result<(), StorageError> {
        let result = sqlx::query(
            "UPDATE accounts SET tmux_pane_id = ? WHERE id = ?",
        )
        .bind(pane_id)
        .bind(account_id)
        .execute(&self.db)
        .await?;

        if result.rows_affected() == 0 {
            return Err(StorageError::NotFound(format!("account {account_id}")));
        }
        Ok(())
    }

    async fn reset_token(&self, account_id: &str) -> Result<Account, StorageError> {
        let new_token = uuid::Uuid::new_v4().to_string();

        let result = sqlx::query(
            "UPDATE accounts SET bearer_token = ? WHERE id = ?",
        )
        .bind(&new_token)
        .bind(account_id)
        .execute(&self.db)
        .await?;

        if result.rows_affected() == 0 {
            return Err(StorageError::NotFound(format!("account {account_id}")));
        }

        let row = sqlx::query(
            "SELECT id, name, display_name, bio, bearer_token, tmux_pane_id, active, created_at FROM accounts WHERE id = ?",
        )
        .bind(account_id)
        .fetch_one(&self.db)
        .await?;

        Ok(row_to_account(&row))
    }

    async fn insert_message(&self, msg: NewMessage) -> Result<Message, StorageError> {
        let mut tx = self.db.begin().await?;
        let msg_id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string();
        let snippet = make_snippet(&msg.body);
        let reply_requested = msg.reply_by.is_some();

        let thread_id = resolve_thread(&mut tx, &msg, &snippet, &now).await?;
        let (stored_body, compressed) = compress_body(&msg.body)?;

        // Insert message row
        sqlx::query(
            "INSERT INTO messages (id, thread_id, from_account, subject, body, snippet, internal_date, in_reply_to, reply_by, reply_requested, source, compressed)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&msg_id)
        .bind(&thread_id)
        .bind(&msg.from_account)
        .bind(&msg.subject)
        .bind(&stored_body)
        .bind(&snippet)
        .bind(&now)
        .bind(&msg.in_reply_to)
        .bind(&msg.reply_by)
        .bind(reply_requested)
        .bind(&msg.source)
        .bind(compressed)
        .execute(&mut *tx)
        .await?;

        insert_recipients_and_labels(&mut tx, &msg_id, &msg).await?;

        let all_recipients: Vec<&str> = msg.to.iter().chain(msg.cc.iter()).map(|s| s.as_str()).collect();
        update_thread_metadata(&mut tx, &thread_id, &msg.from_account, &all_recipients, &snippet, &now).await?;

        // FTS is auto-managed by FULLTEXT index in MySQL — no manual insert needed

        let has_attachments = attach_blobs(&mut tx, &msg_id, &msg.attachments).await?;

        tx.commit().await?;

        Ok(build_sent_message(msg, msg_id, thread_id, snippet, now, has_attachments, reply_requested))
    }

    async fn get_message(&self, id: &str) -> Result<Message, StorageError> {
        let row = sqlx::query(
            "SELECT id, thread_id, from_account, subject, body, snippet, has_attachments, internal_date, in_reply_to, reply_by, reply_requested, source, compressed
             FROM messages WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.db)
        .await?;

        let row = match row {
            Some(r) => r,
            None => return Err(StorageError::NotFound(format!("message {id}"))),
        };

        let msg = row_to_message(&row);

        // Load labels
        let label_rows = sqlx::query(
            "SELECT DISTINCT label FROM message_labels WHERE message_id = ?",
        )
        .bind(&msg.id)
        .fetch_all(&self.db)
        .await?;

        let labels: Vec<String> = label_rows.iter().map(|r| r.get("label")).collect();

        // Load recipients
        let recip_rows = sqlx::query(
            "SELECT account_id, recipient_type FROM message_recipients WHERE message_id = ?",
        )
        .bind(&msg.id)
        .fetch_all(&self.db)
        .await?;

        let recipients: Vec<Recipient> = recip_rows
            .iter()
            .map(|r| Recipient {
                account_id: r.get("account_id"),
                recipient_type: r.get("recipient_type"),
            })
            .collect();

        Ok(Message { labels, recipients, ..msg })
    }

    async fn list_messages(
        &self,
        account_id: &str,
        label: &str,
        max_results: u32,
        page_token: Option<&str>,
    ) -> Result<MessageList, StorageError> {
        let base_sql = "SELECT m.id, m.thread_id, m.from_account, m.subject, m.body, m.snippet, m.has_attachments, m.internal_date, m.in_reply_to, m.reply_by, m.reply_requested, m.source, m.compressed
             FROM messages m
             JOIN message_labels ml ON m.id = ml.message_id
             WHERE ml.account_id = ? AND ml.label = ?";

        let (messages, next_page_token) = run_paginated_list(
            &self.db,
            &PaginatedQuery {
                base_sql,
                date_column: "m.internal_date",
                account_id,
                label,
                page_token: &page_token.map(|s| s.to_string()),
                max_results,
            },
            row_to_message,
            |m| m.internal_date.clone(),
        )
        .await?;

        Ok(MessageList {
            result_size_estimate: messages.len() as u32,
            messages,
            next_page_token,
        })
    }

    async fn delete_message(&self, id: &str) -> Result<(), StorageError> {
        let mut tx = self.db.begin().await?;

        // FTS is auto-managed — no manual delete needed
        sqlx::query("DELETE FROM message_labels WHERE message_id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM message_recipients WHERE message_id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM attachments WHERE message_id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        let result = sqlx::query("DELETE FROM messages WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;

        if result.rows_affected() == 0 {
            return Err(StorageError::NotFound(format!("message {id}")));
        }

        tx.commit().await?;
        Ok(())
    }

    async fn add_labels(
        &self,
        message_id: &str,
        account_id: &str,
        labels: &[String],
    ) -> Result<(), StorageError> {
        for label in labels {
            sqlx::query(
                "INSERT IGNORE INTO message_labels (message_id, account_id, label) VALUES (?, ?, ?)",
            )
            .bind(message_id)
            .bind(account_id)
            .bind(label)
            .execute(&self.db)
            .await?;
        }
        Ok(())
    }

    async fn remove_labels(
        &self,
        message_id: &str,
        account_id: &str,
        labels: &[String],
    ) -> Result<(), StorageError> {
        for label in labels {
            sqlx::query(
                "DELETE FROM message_labels WHERE message_id = ? AND account_id = ? AND label = ?",
            )
            .bind(message_id)
            .bind(account_id)
            .bind(label)
            .execute(&self.db)
            .await?;
        }
        Ok(())
    }

    async fn get_labels(
        &self,
        message_id: &str,
        account_id: &str,
    ) -> Result<Vec<String>, StorageError> {
        let rows = sqlx::query(
            "SELECT label FROM message_labels WHERE message_id = ? AND account_id = ?",
        )
        .bind(message_id)
        .bind(account_id)
        .fetch_all(&self.db)
        .await?;

        Ok(rows.iter().map(|r| r.get("label")).collect())
    }

    async fn list_labels_with_counts(
        &self,
        account_id: &str,
    ) -> Result<Vec<LabelCount>, StorageError> {
        let rows = sqlx::query(
            "SELECT ml.label,
                    COALESCE(l.label_type, 'user') as label_type,
                    COUNT(*) as message_count,
                    CAST(SUM(CASE WHEN EXISTS(
                        SELECT 1 FROM message_labels ul
                        WHERE ul.message_id = ml.message_id
                        AND ul.account_id = ml.account_id
                        AND ul.label = 'UNREAD'
                    ) THEN 1 ELSE 0 END) AS SIGNED) as unread_count
             FROM message_labels ml
             LEFT JOIN labels l ON l.name = ml.label AND (l.account_id IS NULL OR l.account_id = ml.account_id)
             WHERE ml.account_id = ?
             GROUP BY ml.label
             ORDER BY ml.label",
        )
        .bind(account_id)
        .fetch_all(&self.db)
        .await?;

        Ok(rows
            .iter()
            .map(|row| LabelCount {
                name: row.get("label"),
                label_type: row.get("label_type"),
                message_count: row.get::<i64, _>("message_count") as u32,
                unread_count: row.get::<i64, _>("unread_count") as u32,
            })
            .collect())
    }

    async fn attach_blob(
        &self,
        message_id: &str,
        blob_hash: &str,
        filename: &str,
        content_type: &str,
        size: u64,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO attachments (message_id, blob_hash, filename, content_type, size) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(message_id)
        .bind(blob_hash)
        .bind(filename)
        .bind(content_type)
        .bind(size)
        .execute(&self.db)
        .await?;

        sqlx::query("UPDATE messages SET has_attachments = 1 WHERE id = ?")
            .bind(message_id)
            .execute(&self.db)
            .await?;

        Ok(())
    }

    async fn get_attachments(&self, message_id: &str) -> Result<Vec<Attachment>, StorageError> {
        let rows = sqlx::query(
            "SELECT blob_hash, filename, content_type, size FROM attachments WHERE message_id = ?",
        )
        .bind(message_id)
        .fetch_all(&self.db)
        .await?;

        Ok(rows
            .iter()
            .map(|row| Attachment {
                blob_hash: row.get("blob_hash"),
                filename: row.get("filename"),
                content_type: row.get("content_type"),
                size: row.get::<i64, _>("size") as u64,
            })
            .collect())
    }

    async fn create_label(
        &self,
        account_id: &str,
        name: &str,
    ) -> Result<(String, String), StorageError> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO labels (id, account_id, name, label_type) VALUES (?, ?, ?, 'user')",
        )
        .bind(&id)
        .bind(account_id)
        .bind(name)
        .execute(&self.db)
        .await?;

        Ok((id, name.to_string()))
    }

    async fn delete_label(
        &self,
        account_id: &str,
        name: &str,
    ) -> Result<(), StorageError> {
        let result = sqlx::query(
            "DELETE FROM labels WHERE account_id = ? AND name = ? AND label_type = 'user'",
        )
        .bind(account_id)
        .bind(name)
        .execute(&self.db)
        .await?;

        if result.rows_affected() == 0 {
            return Err(StorageError::NotFound(format!("label {name}")));
        }

        sqlx::query("DELETE FROM message_labels WHERE account_id = ? AND label = ?")
            .bind(account_id)
            .bind(name)
            .execute(&self.db)
            .await?;

        Ok(())
    }

    async fn batch_modify_labels(
        &self,
        message_ids: &[String],
        account_id: &str,
        add: &[String],
        remove: &[String],
    ) -> Result<(), StorageError> {
        let mut tx = self.db.begin().await?;
        for msg_id in message_ids {
            for label in add {
                sqlx::query(
                    "INSERT IGNORE INTO message_labels (message_id, account_id, label) VALUES (?, ?, ?)",
                )
                .bind(msg_id)
                .bind(account_id)
                .bind(label)
                .execute(&mut *tx)
                .await?;
            }
            for label in remove {
                sqlx::query(
                    "DELETE FROM message_labels WHERE message_id = ? AND account_id = ? AND label = ?",
                )
                .bind(msg_id)
                .bind(account_id)
                .bind(label)
                .execute(&mut *tx)
                .await?;
            }
        }
        tx.commit().await?;
        Ok(())
    }

    async fn create_list(
        &self,
        name: &str,
        description: &str,
    ) -> Result<String, StorageError> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO lists (id, name, description) VALUES (?, ?, ?)")
            .bind(&id)
            .bind(name)
            .bind(description)
            .execute(&self.db)
            .await?;
        Ok(id)
    }

    async fn subscribe_to_list(
        &self,
        list_id: &str,
        account_id: &str,
    ) -> Result<(), StorageError> {
        sqlx::query("INSERT IGNORE INTO list_members (list_id, account_id) VALUES (?, ?)")
            .bind(list_id)
            .bind(account_id)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    async fn unsubscribe_from_list(
        &self,
        list_id: &str,
        account_id: &str,
    ) -> Result<(), StorageError> {
        sqlx::query("DELETE FROM list_members WHERE list_id = ? AND account_id = ?")
            .bind(list_id)
            .bind(account_id)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    async fn get_list_members(
        &self,
        list_id: &str,
    ) -> Result<Vec<String>, StorageError> {
        let rows = sqlx::query("SELECT account_id FROM list_members WHERE list_id = ?")
            .bind(list_id)
            .fetch_all(&self.db)
            .await?;

        Ok(rows.iter().map(|r| r.get("account_id")).collect())
    }

    async fn get_list_by_name(
        &self,
        name: &str,
    ) -> Result<(String, String, String), StorageError> {
        let row = sqlx::query("SELECT id, name, description FROM lists WHERE name = ?")
            .bind(name)
            .fetch_optional(&self.db)
            .await?;

        match row {
            Some(row) => Ok((row.get("id"), row.get("name"), row.get("description"))),
            None => Err(StorageError::NotFound(format!("list {name}"))),
        }
    }

    async fn get_thread(&self, id: &str) -> Result<Thread, StorageError> {
        let row = sqlx::query(
            "SELECT id, subject, snippet, last_message_at, message_count, participants FROM threads WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.db)
        .await?;

        let row = match row {
            Some(r) => r,
            None => return Err(StorageError::NotFound(format!("thread {id}"))),
        };

        let thread = row_to_thread(&row);

        // Load messages in thread
        let msg_rows = sqlx::query(
            "SELECT id, thread_id, from_account, subject, body, snippet, has_attachments, internal_date, in_reply_to, reply_by, reply_requested, source, compressed
             FROM messages WHERE thread_id = ? ORDER BY internal_date ASC",
        )
        .bind(&thread.id)
        .fetch_all(&self.db)
        .await?;

        let messages: Vec<Message> = msg_rows.iter().map(row_to_message).collect();

        Ok(Thread { messages, ..thread })
    }

    async fn list_threads(
        &self,
        account_id: &str,
        label: &str,
        max_results: u32,
        page_token: Option<&str>,
    ) -> Result<ThreadList, StorageError> {
        let base_sql = "SELECT DISTINCT t.id, t.subject, t.snippet, t.last_message_at, t.message_count, t.participants
             FROM threads t
             JOIN messages m ON m.thread_id = t.id
             JOIN message_labels ml ON ml.message_id = m.id
             WHERE ml.account_id = ? AND ml.label = ?";

        let (threads, next_page_token) = run_paginated_list(
            &self.db,
            &PaginatedQuery {
                base_sql,
                date_column: "t.last_message_at",
                account_id,
                label,
                page_token: &page_token.map(|s| s.to_string()),
                max_results,
            },
            row_to_thread,
            |t| t.last_message_at.clone(),
        )
        .await?;

        Ok(ThreadList {
            result_size_estimate: threads.len() as u32,
            threads,
            next_page_token,
        })
    }

    async fn get_unread_count(&self, account_id: &str) -> Result<u32, StorageError> {
        let row = sqlx::query(
            "SELECT COUNT(*) as cnt FROM message_labels WHERE account_id = ? AND label = 'UNREAD'",
        )
        .bind(account_id)
        .fetch_one(&self.db)
        .await?;

        Ok(row.get::<i64, _>("cnt") as u32)
    }

    async fn get_pending_replies(&self, account_id: &str) -> Result<Vec<PendingReply>, StorageError> {
        let rows = sqlx::query(
            "SELECT m.id, m.from_account, m.subject, m.reply_by, m.internal_date
             FROM messages m
             JOIN message_recipients mr ON m.id = mr.message_id
             WHERE mr.account_id = ? AND m.reply_requested = 1
             AND NOT EXISTS (
                 SELECT 1 FROM messages reply
                 WHERE reply.in_reply_to = m.id AND reply.from_account = ?
             )
             ORDER BY m.reply_by IS NULL, m.reply_by ASC",
        )
        .bind(account_id)
        .bind(account_id)
        .fetch_all(&self.db)
        .await?;

        Ok(rows
            .iter()
            .map(|row| PendingReply {
                message_id: row.get("id"),
                from_account: row.get("from_account"),
                subject: row.get("subject"),
                reply_by: row.get("reply_by"),
                sent_at: row.get("internal_date"),
            })
            .collect())
    }

    async fn label_overdue_messages(&self) -> Result<u32, StorageError> {
        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();

        let rows = sqlx::query(
            "SELECT DISTINCT m.id, mr.account_id
             FROM messages m
             JOIN message_recipients mr ON m.id = mr.message_id
             WHERE m.reply_requested = 1
             AND m.reply_by < ?
             AND NOT EXISTS (
                 SELECT 1 FROM messages reply
                 WHERE reply.in_reply_to = m.id AND reply.from_account = mr.account_id
             )
             AND NOT EXISTS (
                 SELECT 1 FROM message_labels ml
                 WHERE ml.message_id = m.id AND ml.account_id = mr.account_id AND ml.label = 'OVERDUE'
             )",
        )
        .bind(&now)
        .fetch_all(&self.db)
        .await?;

        let count = rows.len() as u32;
        for row in &rows {
            let msg_id: String = row.get("id");
            let acct_id: String = row.get("account_id");
            sqlx::query(
                "INSERT IGNORE INTO message_labels (message_id, account_id, label) VALUES (?, ?, 'OVERDUE')",
            )
            .bind(&msg_id)
            .bind(&acct_id)
            .execute(&self.db)
            .await?;
        }

        Ok(count)
    }

    async fn get_analytics(&self) -> Result<Analytics, StorageError> {
        let row = sqlx::query(
            "SELECT (SELECT COUNT(*) FROM accounts) as total_accounts,
                    (SELECT COUNT(*) FROM messages) as total_messages,
                    (SELECT COUNT(*) FROM threads) as total_threads,
                    (SELECT COUNT(*) FROM blobs) as total_blobs",
        )
        .fetch_one(&self.db)
        .await?;

        let total_accounts: i64 = row.get("total_accounts");
        let total_messages: i64 = row.get("total_messages");
        let total_threads: i64 = row.get("total_threads");
        let total_blobs: i64 = row.get("total_blobs");

        let per_account = query_per_account_stats(&self.db).await?;

        Ok(Analytics {
            total_accounts: total_accounts as u32,
            total_messages: total_messages as u32,
            total_threads: total_threads as u32,
            total_blobs: total_blobs as u32,
            per_account,
        })
    }
}

async fn query_per_account_stats(pool: &DbPool) -> Result<Vec<AccountStats>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT a.id, a.name,
            (SELECT COUNT(*) FROM messages m WHERE m.from_account = a.id) AS sent,
            (SELECT COUNT(DISTINCT mr.message_id) FROM message_recipients mr WHERE mr.account_id = a.id) AS received,
            (SELECT COUNT(DISTINCT m2.thread_id) FROM messages m2 WHERE m2.from_account = a.id AND m2.id = (SELECT MIN(m3.id) FROM messages m3 WHERE m3.thread_id = m2.thread_id)) AS threads_started,
            (SELECT COUNT(*) FROM message_labels ml WHERE ml.account_id = a.id AND ml.label = 'UNREAD') AS unread
         FROM accounts a
         ORDER BY a.name",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .iter()
        .map(|row| AccountStats {
            account_id: row.get("id"),
            account_name: row.get("name"),
            messages_sent: row.get::<i64, _>("sent") as u32,
            messages_received: row.get::<i64, _>("received") as u32,
            threads_started: row.get::<i64, _>("threads_started") as u32,
            unread_count: row.get::<i64, _>("unread") as u32,
        })
        .collect())
}

fn row_to_message(row: &sqlx::mysql::MySqlRow) -> Message {
    let stored_body: String = row.get("body");
    let compressed: bool = row.get::<i32, _>("compressed") != 0;
    Message {
        id: row.get("id"),
        thread_id: row.get("thread_id"),
        from_account: row.get("from_account"),
        subject: row.get("subject"),
        body: decompress_body(&stored_body, compressed),
        snippet: row.get("snippet"),
        has_attachments: row.get::<i32, _>("has_attachments") != 0,
        internal_date: row.get("internal_date"),
        in_reply_to: row.get("in_reply_to"),
        reply_by: row.get("reply_by"),
        reply_requested: row.get::<i32, _>("reply_requested") != 0,
        labels: vec![],
        recipients: vec![],
        source: row.get("source"),
    }
}

fn row_to_thread(row: &sqlx::mysql::MySqlRow) -> Thread {
    let participants_str: serde_json::Value = row.get("participants");
    let participants: Vec<String> =
        serde_json::from_value(participants_str).unwrap_or_default();
    Thread {
        id: row.get("id"),
        subject: row.get("subject"),
        snippet: row.get("snippet"),
        last_message_at: row.get("last_message_at"),
        message_count: row.get::<i32, _>("message_count") as u32,
        participants,
        messages: vec![],
    }
}

fn row_to_account(row: &sqlx::mysql::MySqlRow) -> Account {
    Account {
        id: row.get("id"),
        name: row.get("name"),
        display_name: row.get("display_name"),
        bio: row.get("bio"),
        bearer_token: row.get("bearer_token"),
        tmux_pane_id: row.get("tmux_pane_id"),
        active: row.get::<i32, _>("active") != 0,
        created_at: row.get("created_at"),
    }
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
    if body.chars().count() > 200 {
        format!("{s}...")
    } else {
        s
    }
}

/// Resolve thread ID: use explicit, inherit from parent, or create new.
async fn resolve_thread(
    tx: &mut sqlx::Transaction<'_, sqlx::MySql>,
    msg: &NewMessage,
    snippet: &str,
    now: &str,
) -> Result<String, StorageError> {
    if let Some(ref tid) = msg.thread_id {
        let row = sqlx::query("SELECT COUNT(*) as cnt FROM threads WHERE id = ?")
            .bind(tid)
            .fetch_one(&mut **tx)
            .await?;
        let count: i64 = row.get("cnt");
        if count == 0 {
            return Err(StorageError::NotFound(format!("thread {tid}")));
        }
        Ok(tid.clone())
    } else if let Some(ref reply_to) = msg.in_reply_to {
        let row = sqlx::query("SELECT thread_id FROM messages WHERE id = ?")
            .bind(reply_to)
            .fetch_optional(&mut **tx)
            .await?;
        match row {
            Some(row) => Ok(row.get("thread_id")),
            None => Err(StorageError::NotFound(format!("in_reply_to message {reply_to}"))),
        }
    } else {
        let tid = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO threads (id, subject, snippet, last_message_at, message_count, participants) VALUES (?, ?, ?, ?, 0, '[]')",
        )
        .bind(&tid)
        .bind(&msg.subject)
        .bind(snippet)
        .bind(now)
        .execute(&mut **tx)
        .await?;
        Ok(tid)
    }
}

/// Compress body if > 512 bytes using zstd + base64.
fn compress_body(body: &str) -> Result<(String, bool), StorageError> {
    if body.len() > 512 {
        let encoded = zstd::encode_all(body.as_bytes(), 3)
            .map_err(StorageError::BlobIo)?;
        use base64::Engine;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&encoded);
        Ok((b64, true))
    } else {
        Ok((body.to_string(), false))
    }
}

/// Insert recipients and assign labels (SENT for sender, INBOX+UNREAD for recipients).
async fn insert_recipients_and_labels(
    tx: &mut sqlx::Transaction<'_, sqlx::MySql>,
    msg_id: &str,
    msg: &NewMessage,
) -> Result<(), StorageError> {
    for to in &msg.to {
        sqlx::query(
            "INSERT INTO message_recipients (message_id, account_id, recipient_type) VALUES (?, ?, 'to')",
        )
        .bind(msg_id)
        .bind(to)
        .execute(&mut **tx)
        .await?;
    }
    for cc in &msg.cc {
        sqlx::query(
            "INSERT INTO message_recipients (message_id, account_id, recipient_type) VALUES (?, ?, 'cc')",
        )
        .bind(msg_id)
        .bind(cc)
        .execute(&mut **tx)
        .await?;
    }

    // Sender gets SENT
    sqlx::query(
        "INSERT INTO message_labels (message_id, account_id, label) VALUES (?, ?, 'SENT')",
    )
    .bind(msg_id)
    .bind(&msg.from_account)
    .execute(&mut **tx)
    .await?;

    // Recipients get INBOX + UNREAD
    for recip in msg.to.iter().chain(msg.cc.iter()) {
        sqlx::query(
            "INSERT IGNORE INTO message_labels (message_id, account_id, label) VALUES (?, ?, 'INBOX')",
        )
        .bind(msg_id)
        .bind(recip)
        .execute(&mut **tx)
        .await?;
        sqlx::query(
            "INSERT IGNORE INTO message_labels (message_id, account_id, label) VALUES (?, ?, 'UNREAD')",
        )
        .bind(msg_id)
        .bind(recip)
        .execute(&mut **tx)
        .await?;
    }

    // Custom labels
    for label in &msg.labels {
        sqlx::query(
            "INSERT IGNORE INTO message_labels (message_id, account_id, label) VALUES (?, ?, ?)",
        )
        .bind(msg_id)
        .bind(&msg.from_account)
        .bind(label)
        .execute(&mut **tx)
        .await?;
        if label == "ISSUE" {
            sqlx::query(
                "INSERT IGNORE INTO message_labels (message_id, account_id, label) VALUES (?, ?, 'OPEN')",
            )
            .bind(msg_id)
            .bind(&msg.from_account)
            .execute(&mut **tx)
            .await?;
        }
    }

    Ok(())
}

/// Update thread metadata after inserting a message.
async fn update_thread_metadata(
    tx: &mut sqlx::Transaction<'_, sqlx::MySql>,
    thread_id: &str,
    from: &str,
    recipients: &[&str],
    snippet: &str,
    now: &str,
) -> Result<(), StorageError> {
    let mut parts: Vec<String> = vec![from.to_string()];
    for r in recipients {
        if !parts.contains(&r.to_string()) {
            parts.push(r.to_string());
        }
    }
    let participants_json = serde_json::to_string(&parts).unwrap_or_else(|_| "[]".to_string());

    sqlx::query(
        "UPDATE threads SET snippet = ?, last_message_at = ?, message_count = message_count + 1, participants = ? WHERE id = ?",
    )
    .bind(snippet)
    .bind(now)
    .bind(&participants_json)
    .bind(thread_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// Trim results to max_results, returning a page token from the extra item.
/// The token is the cursor value of the extra (max_results+1th) item, used with
/// `<= token` to ensure it appears on the next page.
fn paginate_results<T, F: FnOnce(&T) -> String>(
    items: &mut Vec<T>,
    max_results: u32,
    token_fn: F,
) -> Option<String> {
    if items.len() > max_results as usize {
        let token = items.last().map(token_fn);
        items.truncate(max_results as usize);
        token
    } else {
        None
    }
}

/// Parameters for a paginated list query.
struct PaginatedQuery<'a> {
    base_sql: &'a str,
    date_column: &'a str,
    account_id: &'a str,
    label: &'a str,
    page_token: &'a Option<String>,
    max_results: u32,
}

/// Execute a paginated list query with dynamic params, page_token filtering, and pagination.
async fn run_paginated_list<T, F, G>(
    pool: &DbPool,
    q: &PaginatedQuery<'_>,
    row_mapper: F,
    token_fn: G,
) -> Result<(Vec<T>, Option<String>), StorageError>
where
    F: Fn(&sqlx::mysql::MySqlRow) -> T,
    G: FnOnce(&T) -> String,
{
    let limit = q.max_results + 1;

    // Build dynamic SQL with string formatting (sqlx doesn't support dynamic column names via bind)
    let query = if q.page_token.is_some() {
        format!(
            "{} AND {} <= ? ORDER BY {} DESC LIMIT ?",
            q.base_sql, q.date_column, q.date_column
        )
    } else {
        format!(
            "{} ORDER BY {} DESC LIMIT ?",
            q.base_sql, q.date_column
        )
    };

    let rows = if let Some(token) = q.page_token {
        sqlx::query(&query)
            .bind(q.account_id)
            .bind(q.label)
            .bind(token)
            .bind(limit)
            .fetch_all(pool)
            .await?
    } else {
        sqlx::query(&query)
            .bind(q.account_id)
            .bind(q.label)
            .bind(limit)
            .fetch_all(pool)
            .await?
    };

    let mut items: Vec<T> = rows.iter().map(&row_mapper).collect();
    let next_page_token = paginate_results(&mut items, q.max_results, token_fn);
    Ok((items, next_page_token))
}

/// Link blob attachments to a message and set has_attachments flag.
async fn attach_blobs(
    tx: &mut sqlx::Transaction<'_, sqlx::MySql>,
    msg_id: &str,
    attachments: &[String],
) -> Result<bool, StorageError> {
    let has_attachments = !attachments.is_empty();
    for blob_hash in attachments {
        sqlx::query("INSERT INTO attachments (message_id, blob_hash) VALUES (?, ?)")
            .bind(msg_id)
            .bind(blob_hash)
            .execute(&mut **tx)
            .await?;
    }
    if has_attachments {
        sqlx::query("UPDATE messages SET has_attachments = 1 WHERE id = ?")
            .bind(msg_id)
            .execute(&mut **tx)
            .await?;
    }
    Ok(has_attachments)
}

/// Build a Message from a NewMessage after insert (for send response).
fn build_sent_message(
    msg: NewMessage,
    id: String,
    thread_id: String,
    snippet: String,
    internal_date: String,
    has_attachments: bool,
    reply_requested: bool,
) -> Message {
    let recipients: Vec<Recipient> = msg.to.iter().map(|id| Recipient {
        account_id: id.clone(),
        recipient_type: "to".to_string(),
    }).chain(msg.cc.iter().map(|id| Recipient {
        account_id: id.clone(),
        recipient_type: "cc".to_string(),
    })).collect();

    let mut labels = vec!["SENT".to_string()];
    labels.extend(msg.labels.clone());

    Message {
        id, thread_id, from_account: msg.from_account, subject: msg.subject,
        body: msg.body, snippet, has_attachments, internal_date,
        in_reply_to: msg.in_reply_to, reply_by: msg.reply_by,
        reply_requested, labels, recipients, source: msg.source,
    }
}
