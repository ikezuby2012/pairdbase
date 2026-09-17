pub mod domain;
pub mod error;
pub mod executor;
pub mod guard;
pub mod handler;
pub mod pool;
pub mod repository;
pub mod session;
pub mod use_case;

use axum::{
    routing::{delete, get, post},
    Router,
};

use crate::state::SharedState;

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/sessions", post(handler::open_session))
        .route("/sessions", get(handler::list_sessions))
        .route("/sessions/{session_id}", delete(handler::close_session))
        .route("/execute", post(handler::execute))
}
