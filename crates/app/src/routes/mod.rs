use crate::state::SharedState;
use aide::axum::{routing::get, ApiRouter};

pub fn router(auth_router: ApiRouter<SharedState>) -> ApiRouter<SharedState> {
    ApiRouter::new()
        .nest("/auth", auth_router)
        .nest(
            "/workspaces/{workspace_id}/connections",
            crate::features::connections::router(),
        )
        .nest("/query", crate::features::query::router())
        .route(
            "/ping",
            get(|| async { "pairdbase is running".to_string() }),
        )
}
