use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;
use schemars::JsonSchema;

use crate::{
    abstractions::ApiResponse, extractors::AuthUser, features::query::error::QueryError, state::{AppState, SharedState},
};

use super::domain::ExecuteRequest;

// ── Open session ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct OpenSessionRequest {
    pub connection_id: Uuid,
    pub workspace_id: Uuid,
}

pub async fn open_session(
    State(state): State<SharedState>,
    auth: AuthUser,
    Json(body): Json<OpenSessionRequest>,
) -> Result<ApiResponse<serde_json::Value>, QueryError> {
    let session = state
        .query
        .open_session(body.connection_id, auth.user_id, auth.org_id)
        .await?;

    Ok(ApiResponse::success(serde_json::json!({
        "session_id": session.session_id,
        "connection_id": session.connection_id,
        "expires_at": session.expires_at,
        "expires_in_secs": session.time_remaining_secs(),
        "db_type": session.db_type,
    }), ""))
}

// ── Close session ─────────────────────────────────────────────────────────────

pub async fn close_session(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(session_id): Path<Uuid>,
) -> Result<StatusCode, QueryError> {
    state.query.close_session(session_id);

    Ok(StatusCode::NO_CONTENT)
}

// ── List sessions ─────────────────────────────────────────────────────────────

pub async fn list_sessions(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> Result<ApiResponse<serde_json::Value>, QueryError> {
    let sessions = state.query.list_sessions(auth.user_id);

    Ok(ApiResponse::success(serde_json::json!(sessions), ""))
}

// ── Execute query ─────────────────────────────────────────────────────────────

pub async fn execute(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(body): Json<ExecuteRequest>,
) -> Result<ApiResponse<serde_json::Value>, QueryError> {
    let events = state
        .query
        .execute(body, auth.user_id, auth.org_id, auth.org_id)
        .await?;

    Ok(ApiResponse::success(serde_json::json!(events), ""))
}
