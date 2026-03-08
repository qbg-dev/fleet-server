use std::path::PathBuf;

/// Server configuration, loaded from environment variables.
///
/// | Variable | Default | Field |
/// |----------|---------|-------|
/// | `BORING_MAIL_BIND` | `0.0.0.0:8025` | `bind_addr` |
/// | `BORING_MAIL_DATA_DIR` | `~/.boring-mail` | `data_dir` |
/// | `BORING_MAIL_ADMIN_TOKEN` | none | `admin_token` |
/// | `BORING_MAIL_REGISTRY` | none | `registry_path` |
#[derive(Debug, Clone)]
pub struct Config {
    /// TCP address to bind the HTTP server to.
    pub bind_addr: String,
    /// Root directory for all persistent data.
    pub data_dir: PathBuf,
    /// Path to the SQLite database file.
    pub db_path: PathBuf,
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
}

impl Config {
    pub fn load() -> Self {
        let data_dir = PathBuf::from(
            std::env::var("BORING_MAIL_DATA_DIR")
                .unwrap_or_else(|_| {
                    dirs().to_string_lossy().to_string()
                }),
        );
        let db_path = data_dir.join("mail.db");
        let blob_dir = data_dir.join("blobs");

        Config {
            bind_addr: std::env::var("BORING_MAIL_BIND")
                .unwrap_or_else(|_| "0.0.0.0:8025".to_string()),
            data_dir,
            db_path,
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
        }
    }
}

fn dirs() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".boring-mail")
}
