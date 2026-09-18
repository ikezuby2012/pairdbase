use aide::{
    axum::ApiRouter,
    openapi::{Info, OpenApi, SecurityScheme, Tag},
    transform::TransformOpenApi,
};
use std::collections::BTreeMap;

pub fn api_docs(api: TransformOpenApi) -> TransformOpenApi {
    api.title("QueryForge API")
        .version("1.0.0")
        .description("Collaborative multi-database developer workspace API")
        .tag(Tag {
            name:        "auth".into(),
            description: Some("Authentication — register, login, OAuth".into()),
            ..Default::default()
        })
        .tag(Tag {
            name:        "connections".into(),
            description: Some("Database connection management".into()),
            ..Default::default()
        })
        .tag(Tag {
            name:        "query".into(),
            description: Some("Query execution".into()),
            ..Default::default()
        })
        .security_scheme(
            "bearer_auth",
            SecurityScheme::Http {
                scheme:        "bearer".into(),
                bearer_format: Some("JWT".into()),
                description:   Some("JWT access token from /auth/login".into()),
                extensions:    Default::default(),
            },
        )
        .default_response_with::<(), _>(|res| {
            res.description("Unexpected error")
        })
}