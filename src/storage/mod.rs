//! Storage traits and implementations.
//!
//! Three traits define the storage contract:
//! - [`DataStore`] — accounts, messages, threads, labels, mailing lists, analytics
//! - [`BlobStore`] — content-addressed blob storage (SHA-256 keys, zstd compression)
//! - [`SearchStore`] — full-text search indexing and querying (FTS5)
//!
//! Implementations: [`sqlite::DoltDataStore`], [`blob::FsBlobStore`], [`fts::SqliteSearchStore`].

pub mod models;
pub mod sqlite;
pub mod blob;
pub mod fts;

use crate::error::StorageError;
use models::*;

/// Core data operations (accounts, messages, threads, labels)
#[allow(async_fn_in_trait)]
pub trait DataStore: Send + Sync + 'static {
    // Accounts
    async fn create_account(&self, name: &str, display_name: Option<&str>, bio: Option<&str>) -> Result<Account, StorageError>;
    async fn get_account_by_id(&self, id: &str) -> Result<Account, StorageError>;
    async fn get_account_by_name(&self, name: &str) -> Result<Account, StorageError>;
    async fn get_account_by_token(&self, token: &str) -> Result<Account, StorageError>;
    async fn list_accounts(&self) -> Result<Vec<Account>, StorageError>;
    async fn update_profile(&self, account_id: &str, display_name: Option<&str>, bio: Option<&str>) -> Result<Account, StorageError>;
    async fn update_pane(&self, account_id: &str, pane_id: &str) -> Result<(), StorageError>;
    async fn reset_token(&self, account_id: &str) -> Result<Account, StorageError>;
    async fn update_session_blob(&self, account_id: &str, blob_hash: &str) -> Result<(), StorageError>;
    async fn get_session_blob_hash(&self, account_name: &str) -> Result<Option<String>, StorageError>;

    // Messages
    async fn insert_message(&self, msg: NewMessage) -> Result<Message, StorageError>;
    async fn get_message(&self, id: &str) -> Result<Message, StorageError>;
    async fn list_messages(&self, account_id: &str, label: &str, max_results: u32, page_token: Option<&str>) -> Result<MessageList, StorageError>;
    async fn delete_message(&self, id: &str) -> Result<(), StorageError>;

    // Labels
    async fn add_labels(&self, message_id: &str, account_id: &str, labels: &[String]) -> Result<(), StorageError>;
    async fn remove_labels(&self, message_id: &str, account_id: &str, labels: &[String]) -> Result<(), StorageError>;
    async fn get_labels(&self, message_id: &str, account_id: &str) -> Result<Vec<String>, StorageError>;
    async fn list_labels_with_counts(&self, account_id: &str) -> Result<Vec<LabelCount>, StorageError>;
    async fn create_label(&self, account_id: &str, name: &str) -> Result<(String, String), StorageError>;
    async fn delete_label(&self, account_id: &str, name: &str) -> Result<(), StorageError>;

    // Attachments
    #[allow(dead_code)]
    async fn attach_blob(&self, message_id: &str, blob_hash: &str, filename: &str, content_type: &str, size: u64) -> Result<(), StorageError>;
    async fn get_attachments(&self, message_id: &str) -> Result<Vec<Attachment>, StorageError>;

    // Batch
    async fn batch_modify_labels(&self, message_ids: &[String], account_id: &str, add: &[String], remove: &[String]) -> Result<(), StorageError>;

    // Threads
    async fn get_thread(&self, id: &str) -> Result<Thread, StorageError>;
    async fn list_threads(&self, account_id: &str, label: &str, max_results: u32, page_token: Option<&str>) -> Result<ThreadList, StorageError>;

    // Mailing lists
    async fn create_list(&self, name: &str, description: &str) -> Result<String, StorageError>;
    async fn subscribe_to_list(&self, list_id: &str, account_id: &str) -> Result<(), StorageError>;
    async fn unsubscribe_from_list(&self, list_id: &str, account_id: &str) -> Result<(), StorageError>;
    async fn get_list_members(&self, list_id: &str) -> Result<Vec<String>, StorageError>;
    async fn get_list_by_name(&self, name: &str) -> Result<(String, String, String), StorageError>; // (id, name, description)

    // Diagnostics
    async fn get_unread_count(&self, account_id: &str) -> Result<u32, StorageError>;
    async fn get_pending_replies(&self, account_id: &str) -> Result<Vec<PendingReply>, StorageError>;

    // Background
    async fn label_overdue_messages(&self) -> Result<u32, StorageError>;

    // Analytics
    async fn get_analytics(&self) -> Result<Analytics, StorageError>;
}

/// Content-addressed blob storage
#[allow(async_fn_in_trait)]
pub trait BlobStore: Send + Sync + 'static {
    async fn store_blob(&self, data: &[u8]) -> Result<BlobMeta, StorageError>;
    async fn get_blob(&self, hash: &str) -> Result<Vec<u8>, StorageError>;
    #[allow(dead_code)]
    async fn blob_exists(&self, hash: &str) -> Result<bool, StorageError>;
}

/// Full-text search
#[allow(async_fn_in_trait)]
pub trait SearchStore: Send + Sync + 'static {
    async fn search(&self, account_id: &str, query: &str, max_results: u32) -> Result<Vec<String>, StorageError>;
    #[allow(dead_code)]
    async fn index_message(&self, id: &str, subject: &str, body: &str) -> Result<(), StorageError>;
    #[allow(dead_code)]
    async fn remove_from_index(&self, id: &str) -> Result<(), StorageError>;
}
