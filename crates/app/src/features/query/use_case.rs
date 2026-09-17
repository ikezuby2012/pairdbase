use chrono::Utc;
use sha2::{Digest, Sha256};
use shared::{ConnectionId, OrgId, QueryHisId, UserId, WorkspaceId};
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

use crate::features::connections::{domain::DbType, use_cases::ConnectionUseCases};

use super::{
    domain::{
        ExecuteRequest, GuardVerdict, HistoryStatus, QueryEvent, QueryHistoryRecord, StatementType,
    },
    error::QueryError,
    executor::QueryExecutor,
    guard,
    pool::{build_execution_pool, ExecutionPoolRegistry},
    repository::QueryRepo,
    session::{QuerySession, SessionRegistry},
};

pub struct QueryUseCases {
    repo: Arc<dyn QueryRepo>,
    executor: Arc<QueryExecutor>,
    sessions: Arc<SessionRegistry>,
    pools: Arc<ExecutionPoolRegistry>,
    connections: Arc<ConnectionUseCases>,
}

impl QueryUseCases {
    pub fn new(
        repo: Arc<dyn QueryRepo>,
        sessions: Arc<SessionRegistry>,
        pools: Arc<ExecutionPoolRegistry>,
        connections: Arc<ConnectionUseCases>,
    ) -> Self {
        let executor = Arc::new(QueryExecutor::new(Arc::clone(&pools)));
        Self {
            repo,
            executor,
            sessions,
            pools,
            connections,
        }
    }

    // ── Open session ──────────────────────────────────────────────────────────

    pub async fn open_session(
        &self,
        connection_id: Uuid,
        user_id: Uuid,
        org_id: Uuid,
    ) -> Result<QuerySession, QueryError> {
        // If a valid session already exists, return it
        if let Some(existing) = self.sessions.get_for_connection(user_id, connection_id) {
            return Ok(existing);
        }

        // Load connection + decrypt credentials
        let conn = self
            .connections
            .get_full(connection_id, org_id)
            .await
            .map_err(|_| QueryError::ConnectionNotFound)?;

        let db_type = conn.db_type.clone();

        // Build execution pool if not already present
        if !self.pools.is_connected(connection_id) {
            let pool = build_execution_pool(&conn)
                .await
                .map_err(|e| QueryError::ExecutionFailed(e))?;

            self.pools.register(connection_id, pool);

            tracing::info!(
                connection_id = %connection_id,
                db_type       = ?db_type,
                "execution pool created"
            );
        }

        // Create session
        let session = self.sessions.create(connection_id, user_id, db_type);

        tracing::info!(
            session_id    = %session.session_id,
            connection_id = %connection_id,
            expires_at    = %session.expires_at,
            "query session opened"
        );

        Ok(session)
    }

    // ── Close session ─────────────────────────────────────────────────────────

    pub fn close_session(&self, session_id: Uuid) {
        self.sessions.expire(session_id);
        tracing::info!(session_id = %session_id, "query session closed");
    }

    // ── Execute query ─────────────────────────────────────────────────────────

    pub async fn execute(
        &self,
        req: ExecuteRequest,
        user_id: Uuid,
        org_id: Uuid,
        organization_id: Uuid,
    ) -> Result<Vec<QueryEvent>, QueryError> {
        let execution_id = Uuid::new_v4();
        let query_hash = hash_query(&req.query);
        let max_rows = req.max_rows.unwrap_or(1_000).min(100_000);
        let timeout = req.timeout_secs.unwrap_or(30).min(300);

        // ── 1. Verify session ─────────────────────────────────────────────────
        let session = match self.sessions.get_for_connection(user_id, req.connection_id) {
            Some(s) if !s.is_expired() => s,
            Some(_) => {
                return Ok(vec![QueryEvent::SessionExpired {
                    connection_id: req.connection_id,
                    message: "Session expired. Please reconnect to continue.".into(),
                }]);
            }
            None => return Err(QueryError::NoSession),
        };

        let db_type = session.db_type.clone();

        // ── 2. Load permission ────────────────────────────────────────────────
        let permission = self.repo.get_permission(req.connection_id, user_id).await?;

        // ── 3. Guard ──────────────────────────────────────────────────────────
        let verdict = guard::evaluate(&req.query, &db_type, &permission);

        match verdict {
            GuardVerdict::Rejected { reason } => {
                let message = reason.message();

                self.repo
                    .save_history(QueryHistoryRecord {
                        id: QueryHisId(Uuid::new_v4()),
                        organization_id: OrgId(organization_id),
                        workspace_id: WorkspaceId(req.workspace_id),
                        connection_id: ConnectionId(req.connection_id),
                        user_id: UserId(user_id),
                        query_text: req.query,
                        query_hash,
                        status: HistoryStatus::Rejected,
                        rejection_reason: Some(message.clone()),
                        duration_ms: None,
                        rows_affected: None,
                        error_message: None,
                        executed_at: Utc::now(),
                        created_at: Utc::now(),
                        created_by: UserId(user_id),
                    })
                    .await?;

                return Ok(vec![QueryEvent::Rejected {
                    execution_id,
                    reason: message,
                }]);
            }

            GuardVerdict::Approved { statement_type } => {
                // ── 4. Execute ────────────────────────────────────────────────
                let start = Instant::now();
                let result = self
                    .executor
                    .run(req.connection_id, &db_type, &req.query, max_rows, timeout)
                    .await;

                let duration_ms = start.elapsed().as_millis() as u64;

                // ── 5. Touch session (extend TTL on activity) ─────────────────
                self.sessions.touch(session.session_id);

                match result {
                    Ok(raw) => {
                        self.repo
                            .save_history(QueryHistoryRecord {
                                id: QueryHisId(Uuid::new_v4()),
                                organization_id: OrgId(organization_id),
                                workspace_id: WorkspaceId(req.workspace_id),
                                connection_id: ConnectionId(req.connection_id),
                                user_id: UserId(user_id),
                                query_text: req.query.clone(),
                                query_hash,
                                status: HistoryStatus::Success,
                                rejection_reason: None,
                                duration_ms: Some(duration_ms as i64),
                                rows_affected: Some(raw.rows.len() as i64),
                                error_message: None,
                                executed_at: Utc::now(),
                                created_at: Utc::now(),
                                created_by: UserId(user_id),
                            })
                            .await?;

                        let truncated = raw.truncated;
                        let rows_affected = raw.rows.len() as u64;
                        let columns = raw.columns.clone();
                        let rows = raw.rows;

                        // Build event sequence
                        let mut events = vec![
                            QueryEvent::Started {
                                execution_id,
                                statement_type: statement_type.clone(),
                            },
                            QueryEvent::Columns {
                                execution_id,
                                columns,
                            },
                        ];

                        // Rows in batches of 500
                        for (offset, chunk) in rows.chunks(500).enumerate() {
                            events.push(QueryEvent::Rows {
                                execution_id,
                                rows: chunk.to_vec(),
                                offset: offset * 500,
                            });
                        }

                        events.push(QueryEvent::Completed {
                            execution_id,
                            rows_affected,
                            duration_ms,
                            truncated,
                        });

                        Ok(events)
                    }

                    Err(e) => {
                        self.repo
                            .save_history(QueryHistoryRecord {
                                id: QueryHisId(Uuid::new_v4()),
                                organization_id: OrgId(organization_id),
                                workspace_id: WorkspaceId(req.workspace_id),
                                connection_id: ConnectionId(req.connection_id),
                                user_id: UserId(user_id),
                                query_text: req.query,
                                query_hash,
                                status: HistoryStatus::Error,
                                rejection_reason: None,
                                duration_ms: Some(duration_ms as i64),
                                rows_affected: None,
                                error_message: Some(e.to_string()),
                                executed_at: Utc::now(),
                                created_at: Utc::now(),
                                created_by: UserId(user_id),
                            })
                            .await?;

                        Ok(vec![QueryEvent::Failed {
                            execution_id,
                            message: e.to_string(),
                            duration_ms,
                        }])
                    }
                }
            }
        }
    }

    // ── Session status ────────────────────────────────────────────────────────

    pub fn list_sessions(&self, user_id: Uuid) -> Vec<QuerySession> {
        self.sessions.list_for_user(user_id)
    }
}

fn hash_query(query: &str) -> String {
    let normalized = query.split_whitespace().collect::<Vec<_>>().join(" ");
    hex::encode(Sha256::digest(normalized.as_bytes()))
}
