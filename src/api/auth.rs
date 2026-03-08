use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts},
};
use crate::error::ApiError;
use crate::storage::models::Account;
use crate::storage::sqlite::DoltDataStore;
use crate::storage::blob::FsBlobStore;
use crate::storage::fts::SqliteSearchStore;
use crate::storage::DataStore;

/// Authenticated account extracted from Bearer token.
pub struct AuthAccount(pub Account);

impl FromRequestParts<AppState> for AuthAccount {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or(ApiError::Unauthorized)?;

        let token = header
            .strip_prefix("Bearer ")
            .ok_or(ApiError::Unauthorized)?;

        let account = state
            .store
            .get_account_by_token(token)
            .await
            .map_err(|_| ApiError::Unauthorized)?;

        if !account.active {
            return Err(ApiError::Forbidden);
        }

        Ok(AuthAccount(account))
    }
}

/// Shared application state.
#[derive(Clone)]
pub struct AppState {
    pub store: DoltDataStore,
    pub search: SqliteSearchStore,
    pub blobs: FsBlobStore,
}
