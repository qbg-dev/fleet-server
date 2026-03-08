use std::net::SocketAddr;

use clap::Parser;
use tracing_subscriber::EnvFilter;

mod cli;
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
    let cli = cli::Cli::parse();
    let config = config::Config::load();

    match cli.command {
        None | Some(cli::Command::Serve) => cmd_serve(config).await,
        Some(cli::Command::Init) => cmd_init(config).await,
        Some(cli::Command::Status) => cmd_status(config).await,
        Some(cli::Command::Accounts) => cmd_accounts(config).await,
    }
}

async fn cmd_serve(config: config::Config) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            EnvFilter::new("boring_mail=debug,tower_http=debug")
        }))
        .init();

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

async fn cmd_init(config: config::Config) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("Initializing boring-mail at {:?}", config.data_dir);
    let _db = db::connection::setup(&config).await?;
    println!("  Database: {:?}", config.db_path);
    println!("  Blobs:    {:?}", config.blob_dir);
    println!("Ready.");
    Ok(())
}

async fn cmd_status(config: config::Config) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use storage::DataStore;

    let db_exists = config.db_path.exists();
    println!("boring-mail status");
    println!("  Data dir: {:?}", config.data_dir);
    println!("  Database: {:?} ({})", config.db_path, if db_exists { "exists" } else { "not found" });
    println!("  Blob dir: {:?} ({})", config.blob_dir, if config.blob_dir.exists() { "exists" } else { "not found" });
    println!("  Bind:     {}", config.bind_addr);

    if db_exists {
        let db = db::connection::setup(&config).await?;
        let store = storage::sqlite::SqliteDataStore::new(db.clone());
        let accounts = store.list_accounts().await?;
        let msg_count: i64 = db.call(|conn| {
            conn.query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0))
                .map_err(tokio_rusqlite::Error::from)
        }).await?;
        let thread_count: i64 = db.call(|conn| {
            conn.query_row("SELECT COUNT(*) FROM threads", [], |r| r.get(0))
                .map_err(tokio_rusqlite::Error::from)
        }).await?;
        println!("  Accounts: {}", accounts.len());
        println!("  Messages: {msg_count}");
        println!("  Threads:  {thread_count}");
    }

    // Check if server is running
    let url = format!("http://{}/health", config.bind_addr);
    match reqwest::get(&url).await {
        Ok(resp) if resp.status().is_success() => println!("  Server:   running at {}", config.bind_addr),
        _ => println!("  Server:   not running"),
    }

    Ok(())
}

async fn cmd_accounts(config: config::Config) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use storage::DataStore;

    if !config.db_path.exists() {
        eprintln!("Database not found at {:?}. Run `boring-mail init` first.", config.db_path);
        std::process::exit(1);
    }

    let db = db::connection::setup(&config).await?;
    let store = storage::sqlite::SqliteDataStore::new(db);
    let accounts = store.list_accounts().await?;

    if accounts.is_empty() {
        println!("No accounts registered.");
        return Ok(());
    }

    println!("{:<36}  {:<20}  {:<20}  {:<6}  {}", "ID", "NAME", "DISPLAY", "ACTIVE", "CREATED");
    println!("{}", "-".repeat(110));
    for a in &accounts {
        println!(
            "{:<36}  {:<20}  {:<20}  {:<6}  {}",
            a.id,
            a.name,
            a.display_name.as_deref().unwrap_or("-"),
            if a.active { "yes" } else { "no" },
            a.created_at,
        );
    }
    println!("\n{} account(s)", accounts.len());

    Ok(())
}
