use aide::{
    axum::{
        routing::{delete_with, get_with, post_with, put_with},
        ApiRouter
    },
    transform::TransformOperation,
};

use axum::{
    routing::{get, post},
    Json, Router,
};

use crate::{abstractions::responses::{ApiErrorResponse, ApiResponseSchema}, extractors::AuthUser};

pub mod connection_error;
pub mod domain;
pub mod drivers;
pub mod handler;
pub mod pool;
pub mod repository;
pub mod use_cases;
pub mod vault;

use crate::state::SharedState;
use connection_error::ConnectionError;
use domain::ConnectionView;

pub fn router() -> ApiRouter<SharedState> {
    ApiRouter::new()
        .api_route(
            "/",
            get_with(handler::list, list_docs).post_with(handler::create, create_docs),
        )
        .api_route(
            "/{id}",
            get_with(handler::get, get_docs)
                .put_with(handler::update, update_docs)
                .delete_with(handler::delete, delete_docs),
        )
        .api_route("/test", post_with(handler::test, test_docs))
        .api_route("/{id}/schema", get_with(handler::schema, schema_docs))
        .api_route(
            "/{id}/schema/refresh",
            post_with(handler::refresh_schema, refresh_schema_docs),
        )
}

// ── Docs functions ────────────────────────────────────────────────────────────

fn list_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List connections")
        .description("Returns all database connections in a workspace.")
        .tag("connections")
        .response::<200, Json<ApiResponseSchema<Vec<ConnectionView>>>>()
        .response_with::<401, Json<ApiErrorResponse>, _>(|r| r.description("Unauthorized"))
        .security_requirement("bearer_auth")
}

fn create_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Create connection")
        .description("Create a new database connection. Credentials are encrypted before storage.")
        .tag("connections")
        .response::<201, Json<ApiResponseSchema<ConnectionView>>>()
        .response_with::<400, Json<ConnectionError>, _>(|r| r.description("Invalid database type"))
        .response_with::<409, Json<ConnectionError>, _>(|r| {
            r.description("Name already exists in workspace")
        })
        .security_requirement("bearer_auth")
}

fn get_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get connection")
        .tag("connections")
        .response::<200, Json<ApiResponseSchema<ConnectionView>>>()
        .response_with::<404, Json<ConnectionError>, _>(|r| r.description("Connection not found"))
        .security_requirement("bearer_auth")
}

fn update_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Update connection")
        .tag("connections")
        .response::<200, Json<ApiResponseSchema<ConnectionView>>>()
        .response_with::<403, Json<ConnectionError>, _>(|r| {
            r.description("Forbidden — not the creator")
        })
        .response_with::<404, Json<ConnectionError>, _>(|r| r.description("Not found"))
        .security_requirement("bearer_auth")
}

fn delete_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Delete connection")
        .tag("connections")
        .response::<200, Json<ApiResponseSchema<bool>>>()
        .response_with::<403, Json<ConnectionError>, _>(|r| r.description("Forbidden"))
        .response_with::<404, Json<ConnectionError>, _>(|r| r.description("Not found"))
        .security_requirement("bearer_auth")
}

fn test_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Test connection credentials")
        .description(
            "Test credentials without saving. \
           Always returns 200 — check the `data` field (true/false). \
           A false result means credentials are wrong or host is unreachable.",
        )
        .tag("connections")
        .response::<200, Json<ApiResponseSchema<bool>>>()
        .response_with::<400, Json<ConnectionError>, _>(|r| r.description("Invalid database type"))
        .security_requirement("bearer_auth")
}

fn schema_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get database schema")
        .description("Returns full schema metadata. Cached in Redis for 5 minutes.")
        .tag("connections")
        .response_with::<200, Json<serde_json::Value>, _>(|r| {
            r.description("Schema metadata — shape varies by database type")
        })
        .response_with::<404, Json<ConnectionError>, _>(|r| r.description("Not found"))
        .response_with::<502, Json<ConnectionError>, _>(|r| {
            r.description("Could not reach database")
        })
        .security_requirement("bearer_auth")
}

fn refresh_schema_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Refresh schema cache")
        .description("Invalidates Redis cache and re-fetches schema from the database.")
        .tag("connections")
        .response_with::<200, Json<serde_json::Value>, _>(|r| {
            r.description("Fresh schema metadata")
        })
        .response_with::<502, Json<ConnectionError>, _>(|r| {
            r.description("Could not reach database")
        })
        .security_requirement("bearer_auth")
}
