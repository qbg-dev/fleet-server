/// Gmail-style query AST.
/// Supports: from:, to:, label:, has:attachment, before:, after:, quoted phrases, free text.
#[derive(Debug, Clone, PartialEq)]
pub enum SearchTerm {
    From(String),
    To(String),
    Label(String),
    HasAttachment,
    Before(String), // ISO date
    After(String),  // ISO date
    Text(String),   // free text / FTS5 query
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchQuery {
    pub terms: Vec<SearchTerm>,
}

impl SearchQuery {
    pub fn parse(input: &str) -> Self {
        let mut terms = Vec::new();
        let mut chars = input.chars().peekable();

        while chars.peek().is_some() {
            // Skip whitespace
            while chars.peek().is_some_and(|c| c.is_whitespace()) {
                chars.next();
            }

            if chars.peek().is_none() {
                break;
            }

            // Check for quoted phrase
            if chars.peek() == Some(&'"') {
                chars.next(); // consume opening quote
                let mut phrase = String::new();
                while let Some(&c) = chars.peek() {
                    if c == '"' {
                        chars.next();
                        break;
                    }
                    phrase.push(c);
                    chars.next();
                }
                if !phrase.is_empty() {
                    terms.push(SearchTerm::Text(format!("\"{phrase}\"")));
                }
                continue;
            }

            // Collect a token
            let mut token = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_whitespace() {
                    break;
                }
                token.push(c);
                chars.next();
            }

            // Parse operator:value pairs
            if let Some((op, val)) = token.split_once(':') {
                match op.to_lowercase().as_str() {
                    "from" => terms.push(SearchTerm::From(val.to_string())),
                    "to" => terms.push(SearchTerm::To(val.to_string())),
                    "label" => terms.push(SearchTerm::Label(val.to_uppercase())),
                    "has" if val.eq_ignore_ascii_case("attachment") => {
                        terms.push(SearchTerm::HasAttachment);
                    }
                    "before" => terms.push(SearchTerm::Before(val.to_string())),
                    "after" => terms.push(SearchTerm::After(val.to_string())),
                    _ => terms.push(SearchTerm::Text(token)),
                }
            } else {
                terms.push(SearchTerm::Text(token));
            }
        }

        SearchQuery { terms }
    }

    pub fn fts_query(&self) -> Option<String> {
        let text_parts: Vec<&str> = self
            .terms
            .iter()
            .filter_map(|t| match t {
                SearchTerm::Text(s) => Some(s.as_str()),
                _ => None,
            })
            .collect();

        if text_parts.is_empty() {
            None
        } else {
            Some(text_parts.join(" "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_text() {
        let q = SearchQuery::parse("deploy production");
        assert_eq!(q.terms, vec![
            SearchTerm::Text("deploy".into()),
            SearchTerm::Text("production".into()),
        ]);
    }

    #[test]
    fn test_parse_operators() {
        let q = SearchQuery::parse("from:agent-1 to:agent-2 label:INBOX");
        assert_eq!(q.terms, vec![
            SearchTerm::From("agent-1".into()),
            SearchTerm::To("agent-2".into()),
            SearchTerm::Label("INBOX".into()),
        ]);
    }

    #[test]
    fn test_parse_mixed() {
        let q = SearchQuery::parse("from:merger deploy has:attachment");
        assert_eq!(q.terms, vec![
            SearchTerm::From("merger".into()),
            SearchTerm::Text("deploy".into()),
            SearchTerm::HasAttachment,
        ]);
    }

    #[test]
    fn test_parse_quoted_phrase() {
        let q = SearchQuery::parse("\"merge request\" from:bot");
        assert_eq!(q.terms, vec![
            SearchTerm::Text("\"merge request\"".into()),
            SearchTerm::From("bot".into()),
        ]);
    }

    #[test]
    fn test_parse_date_range() {
        let q = SearchQuery::parse("after:2026-03-01 before:2026-03-08");
        assert_eq!(q.terms, vec![
            SearchTerm::After("2026-03-01".into()),
            SearchTerm::Before("2026-03-08".into()),
        ]);
    }

    #[test]
    fn test_fts_query_extraction() {
        let q = SearchQuery::parse("from:agent deploy production");
        assert_eq!(q.fts_query(), Some("deploy production".into()));

        let q = SearchQuery::parse("from:agent label:INBOX");
        assert_eq!(q.fts_query(), None);
    }
}
