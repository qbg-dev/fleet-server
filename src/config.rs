use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub bind_addr: String,
    pub data_dir: PathBuf,
    pub db_path: PathBuf,
    pub blob_dir: PathBuf,
    pub admin_token: Option<String>,
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
        }
    }
}

fn dirs() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".boring-mail")
}
