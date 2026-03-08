use crate::db::connection::DbPool;
use crate::error::StorageError;
use crate::storage::SearchStore;

#[derive(Clone)]
pub struct SqliteSearchStore {
    db: DbPool,
}

impl SqliteSearchStore {
    pub fn new(db: DbPool) -> Self {
        Self { db }
    }
}

impl SearchStore for SqliteSearchStore {
    async fn search(
        &self,
        account_id: &str,
        query: &str,
        max_results: u32,
    ) -> Result<Vec<String>, StorageError> {
        let account_id = account_id.to_string();
        let query = query.to_string();

        self.db
            .call(move |conn| {
                // Search FTS5 and filter to messages visible to this account
                let mut stmt = conn.prepare(
                    "SELECT m.id FROM messages m
                     JOIN messages_fts fts ON m.rowid = fts.rowid
                     WHERE messages_fts MATCH ?1
                     AND EXISTS (
                         SELECT 1 FROM message_labels ml
                         WHERE ml.message_id = m.id AND ml.account_id = ?2
                     )
                     ORDER BY rank
                     LIMIT ?3"
                )?;

                let ids: Vec<String> = stmt
                    .query_map(rusqlite::params![query, account_id, max_results], |row| {
                        row.get(0)
                    })?
                    .filter_map(|r| r.ok())
                    .collect();

                Ok(ids)
            })
            .await
            .map_err(StorageError::from)
    }

    async fn index_message(
        &self,
        id: &str,
        subject: &str,
        body: &str,
    ) -> Result<(), StorageError> {
        // Already handled in insert_message transaction
        let _ = (id, subject, body);
        Ok(())
    }

    async fn remove_from_index(&self, id: &str) -> Result<(), StorageError> {
        // Already handled in delete_message transaction
        let _ = id;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema::init_schema;
    use crate::storage::models::NewMessage;
    use crate::storage::sqlite::SqliteDataStore;
    use crate::storage::DataStore;

    async fn setup() -> (SqliteDataStore, SqliteSearchStore) {
        let conn = tokio_rusqlite::Connection::open_in_memory().await.unwrap();
        conn.call(|c| { init_schema(c).unwrap(); Ok(()) }).await.unwrap();
        let store = SqliteDataStore::new(conn.clone());
        let search = SqliteSearchStore::new(conn);
        (store, search)
    }

    #[tokio::test]
    async fn test_fts_search() {
        let (store, search) = setup().await;
        let sender = store.create_account("sender", None).await.unwrap();
        let recipient = store.create_account("recipient", None).await.unwrap();

        store.insert_message(NewMessage {
            from_account: sender.id.clone(),
            to: vec![recipient.id.clone()],
            cc: vec![],
            subject: "Deploy notification".to_string(),
            body: "The deployment to production succeeded".to_string(),
            thread_id: None,
            in_reply_to: None,
            reply_by: None,
            labels: vec![],
            source: None,
        }).await.unwrap();

        store.insert_message(NewMessage {
            from_account: sender.id.clone(),
            to: vec![recipient.id.clone()],
            cc: vec![],
            subject: "Bug report".to_string(),
            body: "Found a bug in the login flow".to_string(),
            thread_id: None,
            in_reply_to: None,
            reply_by: None,
            labels: vec![],
            source: None,
        }).await.unwrap();

        // Search for "deploy"
        let results = search.search(&recipient.id, "deploy", 10).await.unwrap();
        assert_eq!(results.len(), 1);

        // Search for "bug"
        let results = search.search(&recipient.id, "bug", 10).await.unwrap();
        assert_eq!(results.len(), 1);

        // Search should not find messages for unrelated account
        let other = store.create_account("other", None).await.unwrap();
        let results = search.search(&other.id, "deploy", 10).await.unwrap();
        assert_eq!(results.len(), 0);
    }
}
