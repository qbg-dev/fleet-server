use crate::config::Config;
use crate::error::StorageError;
use crate::storage::models::BlobMeta;
use crate::storage::BlobStore;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

#[derive(Clone)]
pub struct FsBlobStore {
    blob_dir: PathBuf,
}

impl FsBlobStore {
    pub fn new(config: &Config) -> Self {
        Self {
            blob_dir: config.blob_dir.clone(),
        }
    }
}

impl BlobStore for FsBlobStore {
    async fn store_blob(&self, data: &[u8]) -> Result<BlobMeta, StorageError> {
        let hash = hex_sha256(data);
        let path = self.blob_dir.join(format!("{hash}.zst"));

        if path.exists() {
            // Already stored (content-addressed dedup)
            return Ok(BlobMeta {
                hash,
                size: data.len() as u64,
                compressed: data.len() > 4096,
            });
        }

        std::fs::create_dir_all(&self.blob_dir)?;

        let compressed = data.len() > 4096;
        if compressed {
            let encoded = zstd::encode_all(data, 3)?;
            std::fs::write(&path, &encoded)?;
        } else {
            // Store uncompressed but with .zst extension for consistency
            std::fs::write(&path, data)?;
        }

        Ok(BlobMeta {
            hash,
            size: data.len() as u64,
            compressed,
        })
    }

    async fn get_blob(&self, hash: &str) -> Result<Vec<u8>, StorageError> {
        let path = self.blob_dir.join(format!("{hash}.zst"));
        if !path.exists() {
            return Err(StorageError::NotFound(format!("blob {hash}")));
        }

        let raw = std::fs::read(&path)?;

        // Try to decompress; if it fails, it was stored uncompressed
        match zstd::decode_all(raw.as_slice()) {
            Ok(decoded) => Ok(decoded),
            Err(_) => Ok(raw),
        }
    }

    async fn blob_exists(&self, hash: &str) -> Result<bool, StorageError> {
        let path = self.blob_dir.join(format!("{hash}.zst"));
        Ok(path.exists())
    }
}

fn hex_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> Config {
        let tmp = tempfile::tempdir().unwrap();
        Config {
            bind_addr: "127.0.0.1:0".to_string(),
            data_dir: tmp.path().to_path_buf(),
            db_path: tmp.path().join("mail.db"),
            blob_dir: tmp.path().join("blobs"),
            admin_token: None,
        }
    }

    #[tokio::test]
    async fn test_store_and_get_small_blob() {
        let config = test_config();
        let store = FsBlobStore::new(&config);

        let data = b"hello world";
        let meta = store.store_blob(data).await.unwrap();
        assert!(!meta.hash.is_empty());
        assert_eq!(meta.size, 11);
        assert!(!meta.compressed); // < 4KB

        let fetched = store.get_blob(&meta.hash).await.unwrap();
        assert_eq!(fetched, data);
    }

    #[tokio::test]
    async fn test_store_large_blob_compressed() {
        let config = test_config();
        let store = FsBlobStore::new(&config);

        // 8KB of repeated data (compresses well)
        let data = vec![42u8; 8192];
        let meta = store.store_blob(&data).await.unwrap();
        assert!(meta.compressed);
        assert_eq!(meta.size, 8192);

        let fetched = store.get_blob(&meta.hash).await.unwrap();
        assert_eq!(fetched, data);
    }

    #[tokio::test]
    async fn test_content_addressed_dedup() {
        let config = test_config();
        let store = FsBlobStore::new(&config);

        let data = b"duplicate data";
        let meta1 = store.store_blob(data).await.unwrap();
        let meta2 = store.store_blob(data).await.unwrap();
        assert_eq!(meta1.hash, meta2.hash);
    }

    #[tokio::test]
    async fn test_blob_not_found() {
        let config = test_config();
        let store = FsBlobStore::new(&config);

        let result = store.get_blob("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_blob_exists() {
        let config = test_config();
        let store = FsBlobStore::new(&config);

        assert!(!store.blob_exists("nonexistent").await.unwrap());

        let meta = store.store_blob(b"test").await.unwrap();
        assert!(store.blob_exists(&meta.hash).await.unwrap());
    }
}
