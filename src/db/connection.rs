use crate::config::Config;
use tokio_rusqlite::Connection;

pub type DbPool = Connection;

pub async fn setup(config: &Config) -> Result<DbPool, Box<dyn std::error::Error + Send + Sync>> {
    // Ensure data directory exists
    std::fs::create_dir_all(&config.data_dir)?;
    std::fs::create_dir_all(&config.blob_dir)?;

    let db_path = config.db_path.clone();
    let conn = Connection::open(db_path).await?;

    // Initialize schema
    conn.call(|conn| {
        crate::db::schema::init_schema(conn)?;
        Ok(())
    })
    .await?;

    tracing::info!("database initialized at {:?}", config.db_path);
    Ok(conn)
}
