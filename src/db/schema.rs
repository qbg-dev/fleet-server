use rusqlite::Connection;

pub fn init_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA busy_timeout = 5000;
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS accounts (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            display_name TEXT,
            bio TEXT,
            bearer_token TEXT NOT NULL UNIQUE,
            tmux_pane_id TEXT,
            active INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
        );

        CREATE TABLE IF NOT EXISTS threads (
            id TEXT PRIMARY KEY,
            subject TEXT NOT NULL DEFAULT '',
            snippet TEXT NOT NULL DEFAULT '',
            last_message_at TEXT NOT NULL,
            message_count INTEGER NOT NULL DEFAULT 0,
            participants TEXT NOT NULL DEFAULT '[]',
            created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
        );

        CREATE TABLE IF NOT EXISTS messages (
            id TEXT PRIMARY KEY,
            thread_id TEXT NOT NULL REFERENCES threads(id),
            from_account TEXT NOT NULL REFERENCES accounts(id),
            subject TEXT NOT NULL DEFAULT '',
            body TEXT NOT NULL DEFAULT '',
            snippet TEXT NOT NULL DEFAULT '',
            has_attachments INTEGER NOT NULL DEFAULT 0,
            internal_date TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
            in_reply_to TEXT,
            reply_by TEXT,
            reply_requested INTEGER NOT NULL DEFAULT 0,
            compressed INTEGER NOT NULL DEFAULT 0,
            source TEXT,
            history_id INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_messages_thread ON messages(thread_id);
        CREATE INDEX IF NOT EXISTS idx_messages_from ON messages(from_account);
        CREATE INDEX IF NOT EXISTS idx_messages_date ON messages(internal_date DESC);

        CREATE TABLE IF NOT EXISTS message_recipients (
            message_id TEXT NOT NULL REFERENCES messages(id),
            account_id TEXT NOT NULL REFERENCES accounts(id),
            recipient_type TEXT NOT NULL DEFAULT 'to',
            PRIMARY KEY (message_id, account_id, recipient_type)
        );
        CREATE INDEX IF NOT EXISTS idx_recipients_account ON message_recipients(account_id);

        CREATE TABLE IF NOT EXISTS labels (
            id TEXT PRIMARY KEY,
            account_id TEXT REFERENCES accounts(id),
            name TEXT NOT NULL,
            label_type TEXT NOT NULL DEFAULT 'user',
            UNIQUE(account_id, name)
        );

        CREATE TABLE IF NOT EXISTS message_labels (
            message_id TEXT NOT NULL REFERENCES messages(id),
            account_id TEXT NOT NULL,
            label TEXT NOT NULL,
            PRIMARY KEY (message_id, account_id, label)
        );
        CREATE INDEX IF NOT EXISTS idx_message_labels_account_label ON message_labels(account_id, label);
        CREATE INDEX IF NOT EXISTS idx_message_labels_label ON message_labels(label);

        CREATE TABLE IF NOT EXISTS attachments (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            message_id TEXT NOT NULL REFERENCES messages(id),
            blob_hash TEXT NOT NULL,
            filename TEXT NOT NULL DEFAULT '',
            content_type TEXT NOT NULL DEFAULT 'application/octet-stream',
            size INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_attachments_message ON attachments(message_id);

        CREATE TABLE IF NOT EXISTS blobs (
            hash TEXT PRIMARY KEY,
            size INTEGER NOT NULL,
            compressed INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
        );

        CREATE TABLE IF NOT EXISTS lists (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            description TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
        );

        CREATE TABLE IF NOT EXISTS list_members (
            list_id TEXT NOT NULL REFERENCES lists(id),
            account_id TEXT NOT NULL REFERENCES accounts(id),
            PRIMARY KEY (list_id, account_id)
        );

        CREATE TABLE IF NOT EXISTS audit_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            actor TEXT NOT NULL,
            action TEXT NOT NULL,
            resource_type TEXT NOT NULL,
            resource_id TEXT NOT NULL,
            details TEXT,
            created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
        );
        CREATE INDEX IF NOT EXISTS idx_audit_created ON audit_log(created_at DESC);

        CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
            subject,
            body,
            content='messages',
            content_rowid='rowid'
        );
        ",
    )?;

    // Migrations for existing databases
    // SQLite ALTER TABLE ADD COLUMN is idempotent-safe: ignore "duplicate column" errors
    let _ = conn.execute("ALTER TABLE accounts ADD COLUMN bio TEXT", []);

    // Seed system labels
    let system_labels = [
        "INBOX", "SENT", "TRASH", "UNREAD", "STARRED", "DRAFT",
        "ISSUE", "OPEN", "IN_PROGRESS", "RESOLVED", "WONTFIX",
        "P0", "P1", "P2", "P3",
        "COMMIT", "BLOCKED", "OVERDUE", "NEEDS_TRIAGE",
    ];

    let mut stmt = conn.prepare(
        "INSERT OR IGNORE INTO labels (id, account_id, name, label_type) VALUES (?1, NULL, ?2, 'system')"
    )?;
    for label in &system_labels {
        let id = label.to_lowercase();
        stmt.execute(rusqlite::params![id, label])?;
    }

    Ok(())
}
