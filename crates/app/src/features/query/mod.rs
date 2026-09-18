use aide::{
    axum::{
        routing::{delete_with, get_with, post_with, put_with},
        ApiRouter,
    },
    transform::TransformOperation,
};

use axum::{
    routing::{delete, get, post},
    Json, Router,
};

use crate::abstractions::responses::{ApiErrorResponse, ApiResponseSchema};

pub mod domain;
pub mod error;
pub mod executor;
pub mod guard;
pub mod handler;
pub mod pool;
pub mod repository;
pub mod session;
pub mod use_case;

use domain::ConnectResult;

use crate::state::SharedState;

pub fn router() -> ApiRouter<SharedState> {
    ApiRouter::new()
        .api_route("/sessions", post_with(handler::open_session, open_session_docs))
        .api_route("/sessions", get_with(handler::list_sessions, list_sessions_docs))
        .api_route("/sessions/{session_id}", delete_with(handler::close_session, close_session_docs))
        .api_route("/execute", post_with(handler::execute, execute_docs))
}

pub fn open_session_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Open query session")
        .description(
            "Opens a query execution session for a database connection. \
             The session is associated with the authenticated user and organization.",
        )
        .tag("query")
        .security_requirement("bearer_auth")
}

pub fn list_sessions_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List query sessions")
        .description(
            "Returns the active query sessions belonging to the authenticated user.",
        )
        .tag("query")
        .security_requirement("bearer_auth")
}

pub fn close_session_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Close query session")
        .description("Closes an active query execution session.")
        .tag("query")
        .security_requirement("bearer_auth")
}

pub fn execute_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Execute query")
        .description(
            "Executes a database query using an active query session and \
             records the execution in query history.",
        )
        .tag("query")
        .security_requirement("bearer_auth")
}
