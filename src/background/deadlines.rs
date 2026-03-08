// Deadline expiry checker — Phase 7
// Background task: check reply_by, auto-label OVERDUE

use std::time::Duration;
use crate::storage::sqlite::SqliteDataStore;
use crate::storage::DataStore;

/// Spawn a background task that checks for overdue messages every 60 seconds.
pub fn spawn_overdue_checker(store: SqliteDataStore) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            match store.label_overdue_messages().await {
                Ok(0) => {}
                Ok(n) => tracing::info!("labeled {n} overdue messages"),
                Err(e) => tracing::warn!("overdue check failed: {e}"),
            }
        }
    });
}
