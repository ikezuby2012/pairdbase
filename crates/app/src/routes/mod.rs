use crate::state::SharedState;
use axum::{routing::get, Router};

pub fn router(auth_router: Router<SharedState>) -> Router<SharedState> {
    Router::new()
        .nest("/auth", auth_router)
        .nest(
            "/workspaces/{workspace_id}/connections",
            crate::features::connections::router(),
        )
        .nest("/api/v1/query", crate::features::query::router())
        .route("/ping", get("pairdbase is running".to_string()))
}
