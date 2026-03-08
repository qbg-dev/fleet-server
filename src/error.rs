use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("database error: {0}")]
    Database(#[from] tokio_rusqlite::Error),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("blob I/O error: {0}")]
    BlobIo(#[from] std::io::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum MessageError {
    #[error("storage: {0}")]
    Storage(#[from] StorageError),
    #[error("invalid recipient: {0}")]
    InvalidRecipient(String),
    #[error("empty recipients")]
    EmptyRecipients,
    #[error("validation: {0}")]
    Validation(String),
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
    #[error("not found: {0}")]
    NotFound(String),
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("internal: {0}")]
    Internal(String),
}

impl From<MessageError> for ApiError {
    fn from(e: MessageError) -> Self {
        match e {
            MessageError::InvalidRecipient(r) => ApiError::BadRequest(format!("invalid recipient: {r}")),
            MessageError::EmptyRecipients => ApiError::BadRequest("empty recipients".into()),
            MessageError::Validation(v) => ApiError::BadRequest(v),
            MessageError::Storage(StorageError::NotFound(n)) => ApiError::NotFound(n),
            MessageError::Storage(e) => ApiError::Internal(e.to_string()),
        }
    }
}

impl From<StorageError> for ApiError {
    fn from(e: StorageError) -> Self {
        match e {
            StorageError::NotFound(n) => ApiError::NotFound(n),
            e => ApiError::Internal(e.to_string()),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized"),
            ApiError::Forbidden => (StatusCode::FORBIDDEN, "forbidden"),
            ApiError::NotFound(_) => (StatusCode::NOT_FOUND, "not found"),
            ApiError::BadRequest(_) => (StatusCode::BAD_REQUEST, "bad request"),
            ApiError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal error"),
        };

        let body = json!({
            "error": {
                "code": status.as_u16(),
                "message": message,
                "details": self.to_string(),
            }
        });

        (status, axum::Json(body)).into_response()
    }
}
