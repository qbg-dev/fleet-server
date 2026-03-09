use crate::config::Config;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions, SqliteJournalMode};
use sqlx::SqlitePool;
use std::str::FromStr;

pub type DbPool = SqlitePool;

pub async fn setup(config: &Config) -> Result<DbPool, Box<dyn std::error::Error + Send + Sync>> {
    // Ensure blob directory exists
    std::fs::create_dir_all(&config.blob_dir)?;

    // Build SQLite path from database_url or default to data_dir/mail.db
    let db_path = if config.database_url.starts_with("sqlite:") {
        config.database_url.clone()
    } else {
        let path = config.data_dir.join("mail.db");
        format!("sqlite:{}?mode=rwc", path.display())
    };

    let opts = SqliteConnectOptions::from_str(&db_path)?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(config.max_db_connections)
        .connect_with(opts)
        .await?;

    // Enable WAL mode for better concurrent read performance
    sqlx::query("PRAGMA journal_mode=WAL").execute(&pool).await?;

    // Initialize schema
    crate::db::schema::init_schema(&pool).await?;

    tracing::info!("database connected to {}", db_path);
    Ok(pool)
}
