use crate::db::connection::DbPool;
use crate::error::StorageError;
use crate::search::filter::CompiledQuery;
use crate::search::parser::SearchQuery;
use crate::storage::SearchStore;

#[derive(Clone)]
pub struct SqliteSearchStore {
    db: DbPool,
}

impl SqliteSearchStore {
    pub fn new(db: DbPool) -> Self {
        Self { db }
    }

    /// Advanced search using Gmail-style query syntax.
    /// Parses the query string, compiles to SQL, and executes.
    pub async fn advanced_search(
        &self,
        account_id: &str,
        query_str: &str,
        max_results: u32,
    ) -> Result<Vec<String>, StorageError> {
        let parsed = SearchQuery::parse(query_str);
        let compiled = CompiledQuery::from_query(&parsed, account_id);
        self.db
            .call(move |conn| {
                let mut sql = String::from("SELECT m.id FROM messages m");

                // Join FTS5 if text search needed
                if let Some(ref fts) = compiled.fts_match {
                    sql.push_str(" JOIN messages_fts fts ON m.rowid = fts.rowid");
                    // We'll add MATCH condition below
                    let _ = fts; // used in conditions
                }

                sql.push_str(" WHERE ");
                let mut all_conditions = compiled.conditions.clone();

                // Add FTS match condition
                let mut all_params = compiled.params.clone();
                if let Some(ref fts) = compiled.fts_match {
                    let idx = all_params.len() + 1;
                    all_conditions.push(format!("messages_fts MATCH ?{idx}"));
                    all_params.push(fts.clone());
                }

                sql.push_str(&all_conditions.join(" AND "));
                sql.push_str(" ORDER BY m.internal_date DESC");

                let limit_idx = all_params.len() + 1;
                sql.push_str(&format!(" LIMIT ?{limit_idx}"));
                all_params.push(max_results.to_string());

                let mut stmt = conn.prepare(&sql)?;
                let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                    all_params.iter().map(|p| p as &dyn rusqlite::types::ToSql).collect();

                let ids: Vec<String> = stmt
                    .query_map(param_refs.as_slice(), |row| row.get(0))?
                    .filter_map(|r| r.ok())
                    .collect();

                Ok(ids)
            })
            .await
            .map_err(StorageError::from)
    }
}

impl SearchStore for SqliteSearchStore {
    async fn search(
        &self,
        account_id: &str,
        query: &str,
        max_results: u32,
    ) -> Result<Vec<String>, StorageError> {
        // Use advanced search which handles both simple FTS and operator queries
        self.advanced_search(account_id, query, max_results).await
    }

    async fn index_message(
        &self,
        id: &str,
        subject: &str,
        body: &str,
    ) -> Result<(), StorageError> {
        let _ = (id, subject, body);
        Ok(())
    }

    async fn remove_from_index(&self, id: &str) -> Result<(), StorageError> {
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
        let sender = store.create_account("sender", None, None).await.unwrap();
        let recipient = store.create_account("recipient", None, None).await.unwrap();

        store.insert_message(NewMessage {
            from_account: sender.id.clone(),
            to: vec![recipient.id.clone()],
            cc: vec![],
            subject: "Deploy notification".to_string(),
            body: "The deployment to production succeeded".to_string(),
            thread_id: None, in_reply_to: None, reply_by: None,
            labels: vec![], source: None, attachments: vec![],
        }).await.unwrap();

        store.insert_message(NewMessage {
            from_account: sender.id.clone(),
            to: vec![recipient.id.clone()],
            cc: vec![],
            subject: "Bug report".to_string(),
            body: "Found a bug in the login flow".to_string(),
            thread_id: None, in_reply_to: None, reply_by: None,
            labels: vec![], source: None, attachments: vec![],
        }).await.unwrap();

        let results = search.search(&recipient.id, "deploy", 10).await.unwrap();
        assert_eq!(results.len(), 1);

        let results = search.search(&recipient.id, "bug", 10).await.unwrap();
        assert_eq!(results.len(), 1);

        let other = store.create_account("other", None, None).await.unwrap();
        let results = search.search(&other.id, "deploy", 10).await.unwrap();
        assert_eq!(results.len(), 0);
    }

    #[tokio::test]
    async fn test_advanced_search_from() {
        let (store, search) = setup().await;
        let alice = store.create_account("alice", None, None).await.unwrap();
        let bob = store.create_account("bob", None, None).await.unwrap();
        let charlie = store.create_account("charlie", None, None).await.unwrap();

        // Alice sends to Charlie
        store.insert_message(NewMessage {
            from_account: alice.id.clone(),
            to: vec![charlie.id.clone()],
            cc: vec![],
            subject: "From Alice".to_string(),
            body: "Hello from Alice".to_string(),
            thread_id: None, in_reply_to: None, reply_by: None,
            labels: vec![], source: None, attachments: vec![],
        }).await.unwrap();

        // Bob sends to Charlie
        store.insert_message(NewMessage {
            from_account: bob.id.clone(),
            to: vec![charlie.id.clone()],
            cc: vec![],
            subject: "From Bob".to_string(),
            body: "Hello from Bob".to_string(),
            thread_id: None, in_reply_to: None, reply_by: None,
            labels: vec![], source: None, attachments: vec![],
        }).await.unwrap();

        // Search for messages from alice
        let results = search.advanced_search(&charlie.id, "from:alice", 10).await.unwrap();
        assert_eq!(results.len(), 1);

        // Search for messages from bob
        let results = search.advanced_search(&charlie.id, "from:bob", 10).await.unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_advanced_search_label() {
        let (store, search) = setup().await;
        let sender = store.create_account("sender", None, None).await.unwrap();
        let recipient = store.create_account("recipient", None, None).await.unwrap();

        let msg = store.insert_message(NewMessage {
            from_account: sender.id.clone(),
            to: vec![recipient.id.clone()],
            cc: vec![],
            subject: "Star me".to_string(),
            body: "Body".to_string(),
            thread_id: None, in_reply_to: None, reply_by: None,
            labels: vec![], source: None, attachments: vec![],
        }).await.unwrap();

        // Add STARRED label
        store.add_labels(&msg.id, &recipient.id, &["STARRED".to_string()]).await.unwrap();

        // Search for starred messages
        let results = search.advanced_search(&recipient.id, "label:STARRED", 10).await.unwrap();
        assert_eq!(results.len(), 1);

        // UNREAD should also work (auto-assigned)
        let results = search.advanced_search(&recipient.id, "label:UNREAD", 10).await.unwrap();
        assert_eq!(results.len(), 1);
    }
}
