use std::path::PathBuf;

/// Server configuration, loaded from environment variables.
///
/// | Variable | Default | Field |
/// |----------|---------|-------|
/// | `BORING_MAIL_BIND` | `0.0.0.0:8025` | `bind_addr` |
/// | `BORING_MAIL_DATABASE_URL` | `sqlite:~/.boring-mail/mail.db?mode=rwc` | `database_url` |
/// | `BORING_MAIL_DATA_DIR` | `~/.boring-mail` | `data_dir` |
/// | `BORING_MAIL_ADMIN_TOKEN` | none | `admin_token` |
/// | `BORING_MAIL_REGISTRY` | none | `registry_path` |
#[derive(Debug, Clone)]
pub struct Config {
    /// TCP address to bind the HTTP server to.
    pub bind_addr: String,
    /// SQLite connection URL.
    pub database_url: String,
    /// Maximum database connections in the pool (default: 5).
    pub max_db_connections: u32,
    /// Root directory for persistent data (blobs, etc.).
    pub data_dir: PathBuf,
    /// Directory for content-addressed blob storage.
    pub blob_dir: PathBuf,
    /// Optional admin bearer token for privileged operations.
    #[allow(dead_code)]
    pub admin_token: Option<String>,
    /// Optional path to worker-fleet registry.json for auto-provisioning accounts.
    pub registry_path: Option<PathBuf>,
    /// Maximum request body size in bytes (default: 10MB).
    pub max_body_size: usize,
    /// Request timeout in seconds (default: 30).
    pub request_timeout_secs: u64,
    /// Per-account rate limit in requests per minute (default: 60). 0 = unlimited.
    pub rate_limit_per_minute: u64,
}

impl Config {
    pub fn load() -> Self {
        let data_dir = PathBuf::from(
            std::env::var("BORING_MAIL_DATA_DIR")
                .unwrap_or_else(|_| {
                    dirs().to_string_lossy().to_string()
                }),
        );
        let blob_dir = data_dir.join("blobs");
        let database_url = std::env::var("BORING_MAIL_DATABASE_URL")
            .unwrap_or_else(|_| {
                let path = data_dir.join("mail.db");
                format!("sqlite:{}?mode=rwc", path.display())
            });

        Config {
            bind_addr: std::env::var("BORING_MAIL_BIND")
                .unwrap_or_else(|_| "0.0.0.0:8025".to_string()),
            database_url,
            max_db_connections: std::env::var("BORING_MAIL_MAX_DB_CONNS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5),
            data_dir,
            blob_dir,
            admin_token: std::env::var("BORING_MAIL_ADMIN_TOKEN").ok(),
            registry_path: std::env::var("BORING_MAIL_REGISTRY").ok().map(PathBuf::from),
            max_body_size: std::env::var("BORING_MAIL_MAX_BODY")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10 * 1024 * 1024), // 10MB
            request_timeout_secs: std::env::var("BORING_MAIL_TIMEOUT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(30),
            rate_limit_per_minute: std::env::var("BORING_MAIL_RATE_LIMIT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(60),
        }
    }
}

fn dirs() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".boring-mail")
}
