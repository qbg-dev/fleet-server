use crate::db::connection::DbPool;
use crate::error::StorageError;
use crate::search::filter::CompiledQuery;
use crate::search::parser::SearchQuery;
use crate::storage::SearchStore;
use sqlx::Row;

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

        let mut sql = String::from("SELECT m.id FROM messages m");

        // For FULLTEXT search in MySQL, we use MATCH...AGAINST on the messages table directly
        // No separate FTS table join needed

        sql.push_str(" WHERE ");
        let mut all_conditions = compiled.conditions.clone();

        // Use LIKE for text search (Dolt doesn't support MATCH...AGAINST IN BOOLEAN MODE yet)
        let mut all_params = compiled.params.clone();
        if let Some(ref fts) = compiled.fts_match {
            all_conditions.push("(m.subject LIKE ? OR m.body LIKE ?)".to_string());
            let pattern = format!("%{fts}%");
            all_params.push(pattern.clone());
            all_params.push(pattern);
        }

        sql.push_str(&all_conditions.join(" AND "));
        sql.push_str(" ORDER BY m.internal_date DESC");
        sql.push_str(" LIMIT ?");
        all_params.push(max_results.to_string());

        // Build the query with dynamic binds
        let mut query = sqlx::query(&sql);
        for param in &all_params {
            query = query.bind(param);
        }

        let rows = query.fetch_all(&self.db).await?;
        let ids: Vec<String> = rows.iter().map(|r| r.get("id")).collect();

        Ok(ids)
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
    use crate::storage::sqlite::DoltDataStore;
    use crate::storage::DataStore;
    use sqlx::mysql::MySqlPoolOptions;

    async fn setup() -> (DoltDataStore, SqliteSearchStore) {
        let base_url = std::env::var("BORING_MAIL_TEST_DB_BASE")
            .unwrap_or_else(|_| "mysql://root@localhost:3307".to_string());
        let db_name = format!("test_fts_{}", uuid::Uuid::new_v4().simple());

        let admin = MySqlPoolOptions::new().max_connections(1).connect(&base_url).await.unwrap();
        sqlx::query(&format!("CREATE DATABASE `{db_name}`")).execute(&admin).await.unwrap();
        admin.close().await;

        let pool = MySqlPoolOptions::new()
            .max_connections(5)
            .connect(&format!("{base_url}/{db_name}"))
            .await.unwrap();
        init_schema(&pool).await.unwrap();

        let store = DoltDataStore::new(pool.clone());
        let search = SqliteSearchStore::new(pool);
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
