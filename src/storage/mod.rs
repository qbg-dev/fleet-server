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
    async fn create_account(&self, name: &str, display_name: Option<&str>) -> Result<Account, StorageError>;
    async fn get_account_by_id(&self, id: &str) -> Result<Account, StorageError>;
    async fn get_account_by_name(&self, name: &str) -> Result<Account, StorageError>;
    async fn get_account_by_token(&self, token: &str) -> Result<Account, StorageError>;
    async fn update_pane(&self, account_id: &str, pane_id: &str) -> Result<(), StorageError>;

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
}

/// Content-addressed blob storage
#[allow(async_fn_in_trait)]
pub trait BlobStore: Send + Sync + 'static {
    async fn store_blob(&self, data: &[u8]) -> Result<BlobMeta, StorageError>;
    async fn get_blob(&self, hash: &str) -> Result<Vec<u8>, StorageError>;
    async fn blob_exists(&self, hash: &str) -> Result<bool, StorageError>;
}

/// Full-text search
#[allow(async_fn_in_trait)]
pub trait SearchStore: Send + Sync + 'static {
    async fn search(&self, account_id: &str, query: &str, max_results: u32) -> Result<Vec<String>, StorageError>;
    async fn index_message(&self, id: &str, subject: &str, body: &str) -> Result<(), StorageError>;
    async fn remove_from_index(&self, id: &str) -> Result<(), StorageError>;
}
