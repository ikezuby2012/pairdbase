use schemars::JsonSchema;
use serde::Serialize;

#[derive(Debug, Serialize, JsonSchema)]
pub struct ApiErrorResponse {
    pub status: String,
    pub message: String,
    pub code: String,
    pub data: Option<serde_json::Value>,
}

/// Generates `IntoResponse` and `aide::OperationOutput` for an API error enum.
///
/// Each variant defines:
///
/// - HTTP status
/// - semantic error code
/// - client-facing message
///
/// Example:
///
/// ```ignore
/// impl_api_error! {
///     WorkspaceError {
///         WorkspaceError::NotFound => (
///             StatusCode::NOT_FOUND,
///             "WORKSPACE_NOT_FOUND",
///             "Workspace not found".to_string()
///         ),
///         WorkspaceError::Internal(_) => (
///             StatusCode::INTERNAL_SERVER_ERROR,
///             "INTERNAL_ERROR",
///             "An internal error occurred".to_string()
///         ),
///     }
/// }
/// ```
#[macro_export]
macro_rules! impl_api_error {
    (
        $enum_ty:ty {
            $(
                $variant:pat => (
                    $status:expr,
                    $code:expr,
                    $message:expr
                )
            ),+ $(,)?
        }
    ) => {
        impl ::axum::response::IntoResponse for $enum_ty {
            fn into_response(
                self,
            ) -> ::axum::response::Response {
                let (status, code, message):
                    (
                        ::axum::http::StatusCode,
                        &'static str,
                        ::std::string::String,
                    )
                    = match &self {
                        $(
                            $variant => (
                                $status,
                                $code,
                                $message,
                            )
                        ),+
                    };

                (
                    status,
                    ::axum::Json(
                        $crate::ApiErrorResponse {
                            status: "error".to_string(),
                            message,
                            code: code.to_string(),
                            data: None,
                        }
                    ),
                )
                    .into_response()
            }
        }

        impl ::aide::OperationOutput for $enum_ty {
            type Inner =
                <::axum::Json<$crate::ApiErrorResponse>
                    as ::aide::OperationOutput>::Inner;

            fn operation_response(
                ctx: &mut ::aide::generate::GenContext,
                operation: &mut ::aide::openapi::Operation,
            ) -> Option<::aide::openapi::Response> {
                <::axum::Json<$crate::ApiErrorResponse>
                    as ::aide::OperationOutput>
                    ::operation_response(ctx, operation)
            }

           fn inferred_responses(ctx: &mut ::aide::generate::GenContext, operation: &mut ::aide::openapi::Operation,) -> Vec<(
                Option<::aide::openapi::StatusCode>,
                ::aide::openapi::Response,
            )> {
                <::axum::Json<$crate::ApiErrorResponse>
                    as ::aide::OperationOutput>
                    ::inferred_responses(ctx, operation)
            }


        }
    };
}
