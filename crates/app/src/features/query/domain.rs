use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared::{ConnectionId, OrgId, QueryHisId, UserId, WorkspaceId};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct ExecuteRequest {
    pub connection_id: Uuid,
    pub workspace_id: Uuid,
    pub query: String,
    pub max_rows: Option<usize>,
    pub timeout_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ColumnMeta {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
}

#[derive(Debug, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum QueryEvent {
    /// Query accepted by guard, execution starting
    Started {
        execution_id: Uuid,
        statement_type: StatementType,
    },
    /// Column metadata — sent before first row batch
    Columns {
        execution_id: Uuid,
        columns: Vec<ColumnMeta>,
    },
    /// A batch of rows
    Rows {
        execution_id: Uuid,
        rows: Vec<Vec<serde_json::Value>>,
        offset: usize,
    },
    /// Execution complete
    Completed {
        execution_id: Uuid,
        rows_affected: u64,
        duration_ms: u64,
        truncated: bool,
    },
    /// Execution failed
    Failed {
        execution_id: Uuid,
        message: String,
        duration_ms: u64,
    },
    /// Query was rejected by the guard before reaching the DB
    Rejected { execution_id: Uuid, reason: String },
    /// Session expired — client must reconnect
    SessionExpired {
        connection_id: Uuid,
        message: String,
    },
    ConnectionLost {
        execution_id: Uuid,
        connection_id: Uuid,
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum StatementType {
    Select,
    Insert,
    Update,
    Delete,
    Ddl,
    Other,
}

#[derive(Debug)]
pub enum GuardVerdict {
    Approved { statement_type: StatementType },
    Rejected { reason: RejectionReason },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RejectionReason {
    InsufficientPermission { statement_type: StatementType },
    UnboundedMutation { statement_type: StatementType },
    InjectionSuspect { detail: String },
    ParseFailure { detail: String },
}

impl RejectionReason {
    pub fn message(&self) -> String {
        match self {
            Self::InsufficientPermission { statement_type } =>
                format!("You do not have permission to run {:?} statements on this connection.", statement_type),
            Self::UnboundedMutation { statement_type } =>
                format!("{:?} without a WHERE clause is blocked. Add a WHERE clause or contact your workspace admin.", statement_type),
            Self::InjectionSuspect { detail } =>
                format!("Query blocked — suspected injection pattern: {}", detail),
            Self::ParseFailure { detail } =>
                format!("Query could not be parsed: {}", detail),
        }
    }
}

// ── Connection permission ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ConnectionPermission {
    pub allow_select: bool,
    pub allow_insert: bool,
    pub allow_update: bool,
    pub allow_delete: bool,
    pub allow_ddl: bool,
}

impl Default for ConnectionPermission {
    fn default() -> Self {
        Self {
            allow_select: true,
            allow_insert: false,
            allow_update: false,
            allow_delete: false,
            allow_ddl: false,
        }
    }
}

#[derive(Debug)]
pub struct QueryHistoryRecord {
    pub id: QueryHisId,
    pub organization_id: OrgId,
    pub workspace_id: WorkspaceId,
    pub connection_id: ConnectionId,
    pub user_id: UserId,
    pub query_text: String,
    pub query_hash: String,
    pub status: HistoryStatus,
    pub rejection_reason: Option<String>,
    pub duration_ms: Option<i64>,
    pub rows_affected: Option<i64>,
    pub error_message: Option<String>,
    pub executed_at: DateTime<Utc>,
    pub created_by: UserId,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub enum HistoryStatus {
    Success,
    Error,
    Rejected,
    Cancelled,
}

impl HistoryStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Error => "error",
            Self::Rejected => "rejected",
            Self::Cancelled => "cancelled",
        }
    }
}

impl From<String> for HistoryStatus {
    fn from(value: String) -> Self {
        match value.as_str() {
            "success" => Self::Success,
            "error" => Self::Error,
            "rejected" => Self::Rejected,
            "cancelled" => Self::Cancelled,
            _ => Self::Error,
        }
    }
}

// impl TryFrom<String> for HistoryStatus {
//     type Error = String;

//     fn try_from(value: String) -> Result<Self, Self::Error> {
//         match value.as_str() {
//             "success" => Ok(Self::Success),
//             "error" => Ok(Self::Error),
//             "rejected" => Ok(Self::Rejected),
//             "cancelled" => Ok(Self::Cancelled),
//             _ => Err(format!("invalid history status: {value}")),
//         }
//     }
// }

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionStatus {
    Connected,
    AlreadyConnected,
}

#[derive(Debug, serde::Serialize)]
pub struct ConnectResult {
    pub connection_id: Uuid,
    pub status: ConnectionStatus,
    pub latency_ms: Option<u64>,
    pub message: String,
}

#[derive(sqlx::FromRow)]
pub struct QueryHistoryRow {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub workspace_id: Uuid,
    pub connection_id: Uuid,
    pub user_id: Uuid,
    pub query_text: String,
    pub query_hash: String,
    pub status: String,
    pub rejection_reason: Option<String>,
    pub duration_ms: Option<i64>,
    pub rows_affected: Option<i64>,
    pub error_message: Option<String>,
    pub executed_at: DateTime<Utc>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_by: Option<Uuid>,
    pub updated_at: Option<DateTime<Utc>>,

    pub is_soft_deleted: bool,
    pub deleted_by: Option<Uuid>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct QueryHistoryView {
    pub id: Uuid,

    pub workspace_id: Uuid,
    pub connection_id: Uuid,

    pub query_text: String,

    pub status: String,

    pub rejection_reason: Option<String>,
    pub duration_ms: Option<i64>,
    pub rows_affected: Option<i64>,
    pub error_message: Option<String>,

    pub executed_at: DateTime<Utc>,

    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
}

impl From<QueryHistoryRow> for QueryHistoryRecord {
    fn from(row: QueryHistoryRow) -> Self {
        Self {
            id: QueryHisId(row.id),

            organization_id: OrgId(row.organization_id),
            workspace_id: WorkspaceId(row.workspace_id),
            connection_id: ConnectionId(row.connection_id),
            user_id: UserId(row.user_id),

            query_text: row.query_text,
            query_hash: row.query_hash,

            status: HistoryStatus::from(row.status),

            rejection_reason: row.rejection_reason,
            duration_ms: row.duration_ms,
            rows_affected: row.rows_affected,
            error_message: row.error_message,

            executed_at: row.executed_at,

            created_by: UserId(row.created_by),
            created_at: row.created_at,
        }
    }
}

// impl TryFrom<QueryHistoryRow> for QueryHistoryRecord {
//     type Error = String;

//     fn try_from(row: QueryHistoryRow) -> Result<Self, Self::Error> {
//         Ok(Self {
//             id: QueryHisId(row.id),
//             organization_id: OrgId(row.organization_id),
//             workspace_id: WorkspaceId(row.workspace_id),
//             connection_id: ConnectionId(row.connection_id),
//             user_id: UserId(row.user_id),

//             query_text: row.query_text,
//             query_hash: row.query_hash,

//             status: HistoryStatus::try_from(row.status)?,

//             rejection_reason: row.rejection_reason,
//             duration_ms: row.duration_ms,
//             rows_affected: row.rows_affected,
//             error_message: row.error_message,

//             executed_at: row.executed_at,

//             created_by: UserId(row.created_by),
//             created_at: row.created_at,
//         })
//     }
// }

impl From<&QueryHistoryRecord> for QueryHistoryView {
    fn from(history: &QueryHistoryRecord) -> Self {
        Self {
            id: history.id.0,
            workspace_id: history.workspace_id.0,
            connection_id: history.connection_id.0,

            query_text: history.query_text.clone(),
            status: history.status.as_str().to_string(),

            rejection_reason: history.rejection_reason.clone(),
            duration_ms: history.duration_ms,
            rows_affected: history.rows_affected,
            error_message: history.error_message.clone(),

            executed_at: history.executed_at,

            created_by: history.created_by.0,
            created_at: history.created_at,
        }
    }
}
