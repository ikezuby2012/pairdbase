use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ApiResponse<T>
where
    T: Serialize,
{
    pub status: String,
    pub message: String,
    pub code: u16,
    pub data: T,
}

impl<T> ApiResponse<T>
where
    T: Serialize,
{
    pub fn success(data: T, message: impl Into<String>) -> Self {
        Self {
            status: "success".to_string(),
            message: message.into(),
            code: 200,
            data,
        }
    }

    pub fn created(data: T, message: impl Into<String>) -> Self {
        Self {
            status: "success".to_string(),
            message: message.into(),
            code: 201,
            data,
        }
    }

    pub fn warning(data: T, message: impl Into<String>, code: u16) -> Self {
        Self {
            status: "warning".to_string(),
            message: message.into(),
            code,
            data,
        }
    }

    pub fn error(data: T, message: impl Into<String>, code: u16) -> Self {
        Self {
            status: "error".to_string(),
            message: message.into(),
            code,
            data,
        }
    }
}

impl<T> IntoResponse for ApiResponse<T>
where
    T: Serialize,
{
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

        (status, Json(self)).into_response()
    }
}
