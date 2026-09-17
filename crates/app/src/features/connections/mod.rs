pub mod connection_error;
pub mod domain;
pub mod drivers;
pub mod handler;
pub mod pool;
pub mod repository;
pub mod use_cases;
pub mod vault;

use crate::state::SharedState;

use axum::{
    routing::{get, post},
    Router,
};

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/", get(handler::list).post(handler::create))
        .route(
            "/{id}",
            get(handler::get)
                .put(handler::update)
                .delete(handler::delete),
        )
        .route("/test", post(handler::test))
        .route("/{id}/schema", get(handler::schema))
        .route("/{id}/schema/refresh", post(handler::refresh_schema))
}

