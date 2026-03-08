use std::net::SocketAddr;
use tracing_subscriber::EnvFilter;

mod config;
mod db;
mod error;
mod storage;
mod api;
mod service;
mod search;
mod delivery;
mod background;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            EnvFilter::new("boring_mail=debug,tower_http=debug")
        }))
        .init();

    let config = config::Config::load();

    // Initialize database
    let db = db::connection::setup(&config).await?;

    // Auto-provision from registry.json if configured
    if let Some(ref registry_path) = config.registry_path {
        let store = storage::sqlite::SqliteDataStore::new(db.clone());
        match service::provision::provision_from_registry(&store, registry_path).await {
            Ok(n) => {
                if n > 0 {
                    tracing::info!("provisioned {n} new mail accounts from registry");
                }
            }
            Err(e) => tracing::warn!("registry provisioning failed: {e}"),
        }
    }

    // Spawn background tasks
    background::deadlines::spawn_overdue_checker(storage::sqlite::SqliteDataStore::new(db.clone()));

    // Build router
    let app = api::router(db.clone(), &config);

    let addr: SocketAddr = config.bind_addr.parse()?;
    tracing::info!("boring-mail listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
