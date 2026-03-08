use sqlx::MySqlPool;

pub async fn init_schema(pool: &MySqlPool) -> Result<(), sqlx::Error> {
    // Accounts
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS accounts (
            id VARCHAR(36) PRIMARY KEY,
            name VARCHAR(256) NOT NULL UNIQUE,
            display_name VARCHAR(256),
            bio TEXT,
            bearer_token VARCHAR(36) NOT NULL UNIQUE,
            tmux_pane_id VARCHAR(64),
            active TINYINT NOT NULL DEFAULT 1,
            created_at VARCHAR(32) NOT NULL DEFAULT (DATE_FORMAT(UTC_TIMESTAMP(6), '%Y-%m-%dT%H:%i:%S.%fZ'))
        )",
    )
    .execute(pool)
    .await?;

    // Threads
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS threads (
            id VARCHAR(36) PRIMARY KEY,
            subject TEXT NOT NULL,
            snippet TEXT NOT NULL,
            last_message_at VARCHAR(32) NOT NULL,
            message_count INT NOT NULL DEFAULT 0,
            participants JSON NOT NULL,
            created_at VARCHAR(32) NOT NULL DEFAULT (DATE_FORMAT(UTC_TIMESTAMP(6), '%Y-%m-%dT%H:%i:%S.%fZ'))
        )",
    )
    .execute(pool)
    .await?;

    // Messages (with FULLTEXT index instead of FTS5)
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS messages (
            id VARCHAR(36) PRIMARY KEY,
            thread_id VARCHAR(36) NOT NULL,
            from_account VARCHAR(36) NOT NULL,
            subject TEXT NOT NULL,
            body MEDIUMTEXT NOT NULL,
            snippet TEXT NOT NULL,
            has_attachments TINYINT NOT NULL DEFAULT 0,
            internal_date VARCHAR(32) NOT NULL DEFAULT (DATE_FORMAT(UTC_TIMESTAMP(6), '%Y-%m-%dT%H:%i:%S.%fZ')),
            in_reply_to VARCHAR(36),
            reply_by VARCHAR(32),
            reply_requested TINYINT NOT NULL DEFAULT 0,
            compressed TINYINT NOT NULL DEFAULT 0,
            source VARCHAR(256),
            history_id BIGINT NOT NULL DEFAULT 0,
            FOREIGN KEY (thread_id) REFERENCES threads(id),
            FOREIGN KEY (from_account) REFERENCES accounts(id),
            FULLTEXT INDEX ft_messages (subject, body)
        )",
    )
    .execute(pool)
    .await?;

    // Message indexes
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_messages_thread ON messages(thread_id)")
        .execute(pool).await.ok(); // OK if exists
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_messages_from ON messages(from_account)")
        .execute(pool).await.ok();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_messages_date ON messages(internal_date)")
        .execute(pool).await.ok();

    // Message recipients
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS message_recipients (
            message_id VARCHAR(36) NOT NULL,
            account_id VARCHAR(36) NOT NULL,
            recipient_type VARCHAR(4) NOT NULL DEFAULT 'to',
            PRIMARY KEY (message_id, account_id, recipient_type),
            FOREIGN KEY (message_id) REFERENCES messages(id),
            FOREIGN KEY (account_id) REFERENCES accounts(id)
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_recipients_account ON message_recipients(account_id)")
        .execute(pool).await.ok();

    // Labels
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS labels (
            id VARCHAR(36) PRIMARY KEY,
            account_id VARCHAR(36),
            name VARCHAR(256) NOT NULL,
            label_type VARCHAR(16) NOT NULL DEFAULT 'user',
            UNIQUE KEY uq_labels (account_id, name),
            FOREIGN KEY (account_id) REFERENCES accounts(id)
        )",
    )
    .execute(pool)
    .await?;

    // Message labels
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS message_labels (
            message_id VARCHAR(36) NOT NULL,
            account_id VARCHAR(36) NOT NULL,
            label VARCHAR(256) NOT NULL,
            PRIMARY KEY (message_id, account_id, label)
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_message_labels_account_label ON message_labels(account_id, label)")
        .execute(pool).await.ok();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_message_labels_label ON message_labels(label)")
        .execute(pool).await.ok();

    // Attachments
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS attachments (
            id INT AUTO_INCREMENT PRIMARY KEY,
            message_id VARCHAR(36) NOT NULL,
            blob_hash VARCHAR(64) NOT NULL,
            filename VARCHAR(256) NOT NULL DEFAULT '',
            content_type VARCHAR(128) NOT NULL DEFAULT 'application/octet-stream',
            size BIGINT NOT NULL DEFAULT 0,
            FOREIGN KEY (message_id) REFERENCES messages(id)
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_attachments_message ON attachments(message_id)")
        .execute(pool).await.ok();

    // Blobs metadata
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS blobs (
            hash VARCHAR(64) PRIMARY KEY,
            size BIGINT NOT NULL,
            compressed TINYINT NOT NULL DEFAULT 0,
            created_at VARCHAR(32) NOT NULL DEFAULT (DATE_FORMAT(UTC_TIMESTAMP(6), '%Y-%m-%dT%H:%i:%S.%fZ'))
        )",
    )
    .execute(pool)
    .await?;

    // Mailing lists
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS lists (
            id VARCHAR(36) PRIMARY KEY,
            name VARCHAR(256) NOT NULL UNIQUE,
            description TEXT NOT NULL,
            created_at VARCHAR(32) NOT NULL DEFAULT (DATE_FORMAT(UTC_TIMESTAMP(6), '%Y-%m-%dT%H:%i:%S.%fZ'))
        )",
    )
    .execute(pool)
    .await?;

    // List members
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS list_members (
            list_id VARCHAR(36) NOT NULL,
            account_id VARCHAR(36) NOT NULL,
            PRIMARY KEY (list_id, account_id),
            FOREIGN KEY (list_id) REFERENCES lists(id),
            FOREIGN KEY (account_id) REFERENCES accounts(id)
        )",
    )
    .execute(pool)
    .await?;

    // Audit log
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS audit_log (
            id INT AUTO_INCREMENT PRIMARY KEY,
            actor VARCHAR(256) NOT NULL,
            action VARCHAR(64) NOT NULL,
            resource_type VARCHAR(64) NOT NULL,
            resource_id VARCHAR(256) NOT NULL,
            details TEXT,
            created_at VARCHAR(32) NOT NULL DEFAULT (DATE_FORMAT(UTC_TIMESTAMP(6), '%Y-%m-%dT%H:%i:%S.%fZ'))
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_audit_created ON audit_log(created_at)")
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
            "INSERT IGNORE INTO labels (id, account_id, name, label_type) VALUES (?, NULL, ?, 'system')",
        )
        .bind(&id)
        .bind(label)
        .execute(pool)
        .await?;
    }

    Ok(())
}
