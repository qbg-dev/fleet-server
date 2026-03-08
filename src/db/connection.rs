use crate::config::Config;
use sqlx::mysql::MySqlPoolOptions;
use sqlx::MySqlPool;

pub type DbPool = MySqlPool;

pub async fn setup(config: &Config) -> Result<DbPool, Box<dyn std::error::Error + Send + Sync>> {
    // Ensure blob directory exists
    std::fs::create_dir_all(&config.blob_dir)?;

    // Auto-create the database if it doesn't exist
    if let Some(slash_pos) = config.database_url.rfind('/') {
        let base_url = &config.database_url[..slash_pos];
        let db_name = &config.database_url[slash_pos + 1..];
        if !db_name.is_empty() {
            let admin = MySqlPoolOptions::new()
                .max_connections(1)
                .connect(base_url)
                .await?;
            sqlx::query(&format!("CREATE DATABASE IF NOT EXISTS `{db_name}`"))
                .execute(&admin)
                .await?;
            admin.close().await;
        }
    }

    let pool = MySqlPoolOptions::new()
        .max_connections(config.max_db_connections)
        .connect(&config.database_url)
        .await?;

    // Initialize schema
    crate::db::schema::init_schema(&pool).await?;

    tracing::info!("database connected to {}", config.database_url);
    Ok(pool)
}
