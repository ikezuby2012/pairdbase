use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditInfo {
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub updated_by: Option<Uuid>,
    pub is_soft_deleted: bool,
    pub deleted_at: Option<DateTime<Utc>>,
}
