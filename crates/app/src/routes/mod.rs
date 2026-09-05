use crate::state::SharedState;
use axum::{
    routing::{delete, get, post, put},
    Router,
};

pub fn router(auth_router: Router<SharedState>) -> Router<SharedState> {
    Router::new()
        .nest("/auth", auth_router)
        .route("/ping", get("pairdbase is running".to_string()))
}
