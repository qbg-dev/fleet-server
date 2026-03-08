use crate::search::parser::{SearchQuery, SearchTerm};

/// Compiled search query ready for SQL execution.
pub struct CompiledQuery {
    /// SQL WHERE clause fragments (AND-joined)
    pub conditions: Vec<String>,
    /// Bound parameter values in order
    pub params: Vec<String>,
    /// FTS5 MATCH query if text search is needed
    pub fts_match: Option<String>,
}

impl CompiledQuery {
    pub fn from_query(query: &SearchQuery, account_id: &str) -> Self {
        let mut conditions = Vec::new();
        let mut params = Vec::new();

        // Always scope to account (account_id is always first param)
        conditions.push(
            "EXISTS (SELECT 1 FROM message_labels ml WHERE ml.message_id = m.id AND ml.account_id = ?)".to_string()
        );
        params.push(account_id.to_string());

        for term in &query.terms {
            match term {
                SearchTerm::From(name) => {
                    conditions.push(
                        "EXISTS (SELECT 1 FROM accounts a WHERE a.id = m.from_account AND a.name = ?)".to_string()
                    );
                    params.push(name.clone());
                }
                SearchTerm::To(name) => {
                    conditions.push(
                        "EXISTS (SELECT 1 FROM message_recipients mr JOIN accounts a ON a.id = mr.account_id WHERE mr.message_id = m.id AND a.name = ?)".to_string()
                    );
                    params.push(name.clone());
                }
                SearchTerm::Label(label) => {
                    // For label filter, we need to reference the account_id again.
                    // We push it as an additional param since MySQL doesn't support back-references.
                    conditions.push(
                        "EXISTS (SELECT 1 FROM message_labels ml2 WHERE ml2.message_id = m.id AND ml2.account_id = ? AND ml2.label = ?)".to_string()
                    );
                    params.push(account_id.to_string());
                    params.push(label.clone());
                }
                SearchTerm::HasAttachment => {
                    conditions.push("m.has_attachments = 1".to_string());
                }
                SearchTerm::Before(date) => {
                    conditions.push("m.internal_date < ?".to_string());
                    params.push(date.clone());
                }
                SearchTerm::After(date) => {
                    conditions.push("m.internal_date > ?".to_string());
                    params.push(date.clone());
                }
                SearchTerm::Text(_) => {
                    // Handled via FULLTEXT MATCH...AGAINST
                }
            }
        }

        let fts_match = query.fts_query();

        CompiledQuery {
            conditions,
            params,
            fts_match,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::parser::SearchQuery;

    #[test]
    fn test_compile_basic() {
        let q = SearchQuery::parse("from:agent-1 deploy");
        let compiled = CompiledQuery::from_query(&q, "account-123");

        assert_eq!(compiled.params[0], "account-123");
        assert_eq!(compiled.params[1], "agent-1");
        assert_eq!(compiled.fts_match, Some("deploy".into()));
        assert!(compiled.conditions.len() >= 2);
    }

    #[test]
    fn test_compile_label_filter() {
        let q = SearchQuery::parse("label:STARRED");
        let compiled = CompiledQuery::from_query(&q, "account-123");

        assert!(compiled.conditions.iter().any(|c| c.contains("ml2.label")));
        // Label condition pushes account_id again + label value
        assert_eq!(compiled.params[1], "account-123");
        assert_eq!(compiled.params[2], "STARRED");
    }

    #[test]
    fn test_compile_date_range() {
        let q = SearchQuery::parse("after:2026-03-01 before:2026-03-08");
        let compiled = CompiledQuery::from_query(&q, "acc");

        assert!(compiled.conditions.iter().any(|c| c.contains("internal_date >")));
        assert!(compiled.conditions.iter().any(|c| c.contains("internal_date <")));
    }
}
