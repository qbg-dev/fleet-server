use sqlx::SqlitePool;

pub async fn init_schema(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    // Accounts
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS accounts (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            display_name TEXT,
            bio TEXT,
            bearer_token TEXT NOT NULL UNIQUE,
            tmux_pane_id TEXT,
            active INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%f', 'now') || 'Z')
        )",
    )
    .execute(pool)
    .await?;

    // Threads
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS threads (
            id TEXT PRIMARY KEY,
            subject TEXT NOT NULL,
            snippet TEXT NOT NULL,
            last_message_at TEXT NOT NULL,
            message_count INTEGER NOT NULL DEFAULT 0,
            participants TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%f', 'now') || 'Z')
        )",
    )
    .execute(pool)
    .await?;

    // Messages
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS messages (
            id TEXT PRIMARY KEY,
            thread_id TEXT NOT NULL REFERENCES threads(id),
            from_account TEXT NOT NULL REFERENCES accounts(id),
            subject TEXT NOT NULL,
            body TEXT NOT NULL,
            snippet TEXT NOT NULL,
            has_attachments INTEGER NOT NULL DEFAULT 0,
            internal_date TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%f', 'now') || 'Z'),
            in_reply_to TEXT,
            reply_by TEXT,
            reply_requested INTEGER NOT NULL DEFAULT 0,
            compressed INTEGER NOT NULL DEFAULT 0,
            source TEXT,
            history_id INTEGER NOT NULL DEFAULT 0
        )",
    )
    .execute(pool)
    .await?;

    // Message indexes
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_messages_thread ON messages(thread_id)")
        .execute(pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_messages_from ON messages(from_account)")
        .execute(pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_messages_date ON messages(internal_date)")
        .execute(pool).await?;

    // Message recipients
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS message_recipients (
            message_id TEXT NOT NULL REFERENCES messages(id),
            account_id TEXT NOT NULL REFERENCES accounts(id),
            recipient_type TEXT NOT NULL DEFAULT 'to',
            PRIMARY KEY (message_id, account_id, recipient_type)
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_recipients_account ON message_recipients(account_id)")
        .execute(pool).await?;

    // Labels
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS labels (
            id TEXT PRIMARY KEY,
            account_id TEXT,
            name TEXT NOT NULL,
            label_type TEXT NOT NULL DEFAULT 'user',
            UNIQUE (account_id, name),
            FOREIGN KEY (account_id) REFERENCES accounts(id)
        )",
    )
    .execute(pool)
    .await?;

    // Message labels
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS message_labels (
            message_id TEXT NOT NULL,
            account_id TEXT NOT NULL,
            label TEXT NOT NULL,
            PRIMARY KEY (message_id, account_id, label)
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_message_labels_account_label ON message_labels(account_id, label)")
        .execute(pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_message_labels_label ON message_labels(label)")
        .execute(pool).await?;

    // Attachments
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS attachments (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            message_id TEXT NOT NULL REFERENCES messages(id),
            blob_hash TEXT NOT NULL,
            filename TEXT NOT NULL DEFAULT '',
            content_type TEXT NOT NULL DEFAULT 'application/octet-stream',
            size INTEGER NOT NULL DEFAULT 0
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_attachments_message ON attachments(message_id)")
        .execute(pool).await?;

    // Blobs metadata
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS blobs (
            hash TEXT PRIMARY KEY,
            size INTEGER NOT NULL,
            compressed INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%f', 'now') || 'Z')
        )",
    )
    .execute(pool)
    .await?;

    // Mailing lists
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS lists (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            description TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%f', 'now') || 'Z')
        )",
    )
    .execute(pool)
    .await?;

    // List members
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS list_members (
            list_id TEXT NOT NULL REFERENCES lists(id),
            account_id TEXT NOT NULL REFERENCES accounts(id),
            PRIMARY KEY (list_id, account_id)
        )",
    )
    .execute(pool)
    .await?;

    // Audit log
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS audit_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            actor TEXT NOT NULL,
            action TEXT NOT NULL,
            resource_type TEXT NOT NULL,
            resource_id TEXT NOT NULL,
            details TEXT,
            created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%f', 'now') || 'Z')
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_audit_created ON audit_log(created_at)")
        .execute(pool).await?;

    // FTS5 virtual table for full-text search
    sqlx::query(
        "CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
            subject, body, content='messages', content_rowid='rowid'
        )",
    )
    .execute(pool)
    .await
    .ok(); // OK if already exists with different schema

    // Session file storage — add columns if not present (migration-safe)
    sqlx::query("ALTER TABLE accounts ADD COLUMN session_blob_hash TEXT")
        .execute(pool).await.ok();
    sqlx::query("ALTER TABLE accounts ADD COLUMN session_synced_at TEXT")
        .execute(pool).await.ok();

    // Seed system labels
    let system_labels = [
        "INBOX", "SENT", "TRASH", "UNREAD", "STARRED", "DRAFT",
        "ISSUE", "OPEN", "IN_PROGRESS", "RESOLVED", "WONTFIX",
        "P0", "P1", "P2", "P3",
        "COMMIT", "BLOCKED", "OVERDUE", "NEEDS_TRIAGE",
    ];

    for label in &system_labels {
        let id = label.to_lowercase();
        sqlx::query(
            "INSERT OR IGNORE INTO labels (id, account_id, name, label_type) VALUES (?, NULL, ?, 'system')",
        )
        .bind(&id)
        .bind(label)
        .execute(pool)
        .await?;
    }

    Ok(())
}
