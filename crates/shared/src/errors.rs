use thiserror::Error;

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("not found")]
    NotFound,
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("permission denied")]
    Forbidden,
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("upstream failure: {0}")]
    Upstream(String),
}
