use axum::{routing::{delete, get, post, put}, Router};
use crate::state::SharedState;

pub fn router() -> Router<SharedState> {
    Router::new()
        // .route("/health", get(crate::routes::health::health_check))
        // .route("/api/v1/users", post(crate::routes::users::create_user))
        // .route("/api/v1/users/:user_id", get(crate::routes::users::get_user))
        // .route("/api/v1/users/:user_id", put(crate::routes::users::update_user))
        // .route("/api/v1/users/:user_id", delete(crate::routes::users::delete_user))
}