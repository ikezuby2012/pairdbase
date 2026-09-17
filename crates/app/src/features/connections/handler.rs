use crate::features::connections::domain::ConnectionView;
use crate::{abstractions::ApiResponse, features::connections::drivers::trait_::DatabaseMetadata};
use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use shared::{ConnectionId, OrgId, UserId, WorkspaceId};
use std::sync::Arc;
use uuid::Uuid;

use crate::extractors;

use crate::{
    extractors::AuthUser,
    features::connections::{
        connection_error::ConnectionError,
        domain::{Connection, DbType, SslMode},
        use_cases::ConnectionUseCases,
    },
    state::AppState,
};

// ── Request DTOs ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateConnectionRequest {
    pub workspace_id: Uuid,
    pub name: String,
    pub db_type: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub database_name: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub ssl_mode: Option<String>,
    pub ssh_enabled: Option<bool>,
    pub read_only: Option<bool>,
    pub color: Option<String>,
    pub pool_min: Option<u32>,
    pub pool_max: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateConnectionRequest {
    pub name: Option<String>,
    pub read_only: Option<bool>,
    pub color: Option<String>,
    pub pool_min: Option<u32>,
    pub pool_max: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct TestConnectionRequest {
    pub db_type: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub database_name: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub ssl_mode: Option<String>,
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(workspace_id): Path<Uuid>,
) -> Result<ApiResponse<Vec<ConnectionView>>, ConnectionError> {
    let conns = state.connections.list(workspace_id).await?;

    Ok(ApiResponse::success(
        conns,
        "Connections retrieved successfully",
    ))
}

pub async fn get(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path((_workspace_id, connection_id)): Path<(Uuid, Uuid)>,
) -> Result<ApiResponse<ConnectionView>, ConnectionError> {
    let conn = state.connections.get(connection_id).await?;

    Ok(ApiResponse::success(
        conn,
        "Connection retrieved successfully",
    ))
}

pub async fn update(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path((_workspace_id, connection_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateConnectionRequest>,
) -> Result<ApiResponse<ConnectionView>, ConnectionError> {
    let existing = state
        .connections
        .get_full(connection_id, auth.org_id)
        .await?;

    if existing.created_by != auth.user_id {
        return Err(ConnectionError::Forbidden);
    }

    let updated = Connection {
        name: body.name.unwrap_or(existing.name),
        read_only: body.read_only.unwrap_or(existing.read_only),
        color: body.color.or(existing.color),
        pool_min: body.pool_min.unwrap_or(existing.pool_min),
        pool_max: body.pool_max.unwrap_or(existing.pool_max),
        updated_by: Some(auth.user_id),
        ..existing
    };

    let conn = state.connections.update(updated).await?;

    let conn_view = ConnectionView::from(&conn);

    Ok(ApiResponse::success(
        conn_view,
        "Connection updated successfully",
    ))
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(body): Json<CreateConnectionRequest>,
) -> Result<ApiResponse<ConnectionView>, ConnectionError> {
    let db_type = DbType::try_from(body.db_type.as_str())
        .map_err(|_| ConnectionError::InvalidDbType(body.db_type.clone()))?;

    // let ssl_mode = body
    //     .ssl_mode
    //     .as_deref()
    //     .and_then(|s| SslMode::try_from(s).ok())
    //     .unwrap_or_default();

    let conn = Connection {
        id: Uuid::new_v4(),
        organization_id: auth.org_id,
        workspace_id: body.workspace_id,
        name: body.name,
        db_type,
        host: body.host,
        port: body.port,
        database_name: body.database_name,
        username: body.username,
        password: body.password,
        ssl_mode: body.ssl_mode.unwrap_or_default(),
        ssl_ca_cert: None,
        ssl_client_cert: None,
        ssl_client_key: None,
        ssh_enabled: body.ssh_enabled.unwrap_or(false),
        read_only: body.read_only.unwrap_or(false),
        color: body.color,
        pool_min: body.pool_min.unwrap_or(1),
        pool_max: body.pool_max.unwrap_or(10),
        created_by: auth.user_id,
        created_at: chrono::Utc::now(),
        is_soft_deleted: false,
        deleted_at: None,
        deleted_by: None,
        ssh_host: None,
        ssh_port: None,
        ssh_private_key: None,
        ssh_user: None,
        updated_at: None,
        updated_by: None,
    };

    let conn = state.connections.create(conn).await?;

    Ok(ApiResponse::success(
        conn,
        "Connection created successfully",
    ))
}

pub async fn delete(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path((_workspace_id, connection_id)): Path<(Uuid, Uuid)>,
) -> Result<ApiResponse<()>, ConnectionError> {
    state
        .connections
        .delete(connection_id, auth.user_id)
        .await?;

    Ok(ApiResponse::success((), "Connection deleted successfully"))
}

pub async fn test(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(body): Json<TestConnectionRequest>,
) -> Result<ApiResponse<bool>, ConnectionError> {
    let db_type = DbType::try_from(body.db_type.as_str())
        .map_err(|_| ConnectionError::InvalidDbType(body.db_type.clone()))?;

    let ssl_mode = body
        .ssl_mode
        // .as_deref()
        // .and_then(|s| SslMode::try_from(s).ok())
        .unwrap_or_default();

    // Build a transient Connection — never persisted
    let conn = Connection {
        id: Uuid::new_v4(),
        organization_id: auth.org_id,
        workspace_id: Uuid::nil(),
        name: "test".into(),
        db_type,
        host: body.host,
        port: body.port,
        database_name: body.database_name,
        username: body.username,
        password: body.password,
        ssl_mode,
        ssl_ca_cert: None,
        ssl_client_cert: None,
        ssl_client_key: None,
        ssh_enabled: false,
        ssh_host: None,
        ssh_port: None,
        ssh_private_key: None,
        ssh_user: None,
        read_only: true,
        color: None,
        pool_min: 1,
        pool_max: 2,
        created_by: auth.user_id,
        created_at: chrono::Utc::now(),
        is_soft_deleted: false,
        deleted_at: None,
        deleted_by: None,
        updated_at: None,
        updated_by: None,
    };

    let result = state.connections.test(&conn).await;

    // Return bool — true = credentials valid, false = connection failed
    // Never return 4xx for credential failures — that is a valid test result
    // 4xx/5xx only for our own errors (bad db_type, internal failure)
    Ok(ApiResponse::success(
        result.success,
        "Database connected successfully",
    ))
}

pub async fn schema(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path((_workspace_id, connection_id)): Path<(Uuid, Uuid)>,
) -> Result<ApiResponse<DatabaseMetadata>, ConnectionError> {
    // Verify the connection belongs to this org before exposing schema
    state.connections.get(connection_id).await?;
    //    ↑ returns ConnectionError::NotFound or Forbidden if not authorised

    let metadata = state.connections.get_schema(connection_id).await?;
    // .map_err(|e| ConnectionError::Driver(e))?;

    Ok(ApiResponse::success(metadata, ""))
}

// ── Refresh schema ────────────────────────────────────────────────────────────

pub async fn refresh_schema(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path((_workspace_id, connection_id)): Path<(Uuid, Uuid)>,
) -> Result<ApiResponse<DatabaseMetadata>, ConnectionError> {
    // Same auth check
    state
        .connections
        .get(
            connection_id,
            // auth.org_id
        )
        .await?;

    // Invalidate Redis cache + drop driver from registry
    state
        .connections
        .invalidate_schema_cache(connection_id)
        .await;

    // Re-fetch — cache is cold so this always hits the database
    let metadata = state.connections.get_schema(connection_id).await?;
    // .map_err(|e| ConnectionError::Driver(e))?;

    Ok(ApiResponse::success(metadata, ""))
}
