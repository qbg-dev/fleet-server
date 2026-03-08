use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use crate::api::auth::{AppState, AuthAccount};
use crate::error::ApiError;
use crate::storage::BlobStore;
use serde_json::json;

/// POST /api/blobs — upload a blob (raw bytes)
pub async fn upload_blob(
    _auth: AuthAccount,
    State(state): State<AppState>,
    body: Bytes,
) -> Result<Json<serde_json::Value>, ApiError> {
    if body.is_empty() {
        return Err(ApiError::BadRequest("empty body".into()));
    }

    let meta = state
        .blobs
        .store_blob(&body)
        .await
        .map_err(ApiError::from)?;

    Ok(Json(json!({
        "hash": meta.hash,
        "size": meta.size,
        "compressed": meta.compressed,
    })))
}

/// GET /api/blobs/:hash — download a blob
pub async fn download_blob(
    _auth: AuthAccount,
    State(state): State<AppState>,
    Path(hash): Path<String>,
) -> Result<Response, ApiError> {
    let data = state
        .blobs
        .get_blob(&hash)
        .await
        .map_err(ApiError::from)?;

    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/octet-stream")],
        data,
    )
        .into_response())
}
