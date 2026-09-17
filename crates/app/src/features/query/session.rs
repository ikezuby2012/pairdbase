use chrono::{DateTime, Duration, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tokio::time;
use uuid::Uuid;

use crate::features::connections::domain::DbType;

#[derive(Debug, Clone)]
pub struct ConnectionSlot {
    pub connection_id: Uuid,
    pub user_id: Uuid,
    pub db_type: DbType,
}

pub struct ConnectionRegistry {
    slots: DashMap<Uuid, ConnectionSlot>,
}

impl ConnectionRegistry {
    pub fn new() -> Self {
        Self {
            slots: DashMap::new(),
        }
    }

    pub fn register(&self, slot: ConnectionSlot) {
        tracing::info!(
            connection_id = %slot.connection_id,
            db_type       = ?slot.db_type,
            user_id       = %slot.user_id,
            "connection slot registered"
        );
        self.slots.insert(slot.connection_id, slot);
    }

    pub fn get(&self, connection_id: Uuid) -> Option<ConnectionSlot> {
        self.slots.get(&connection_id).map(|s| s.clone())
    }

    pub fn remove(&self, connection_id: Uuid) {
        self.slots.remove(&connection_id);
        tracing::info!(connection_id = %connection_id, "connection slot removed");
    }

    pub fn is_connected(&self, connection_id: Uuid) -> bool {
        self.slots.contains_key(&connection_id)
    }

    pub fn list_for_user(&self, user_id: Uuid) -> Vec<ConnectionSlot> {
        self.slots
            .iter()
            .filter(|s| s.user_id == user_id)
            .map(|s| s.clone())
            .collect()
    }
}

pub fn ttl_for(db_type: &DbType) -> Duration {
    match db_type {
        DbType::Oracle => Duration::minutes(15),
        DbType::SqlServer => Duration::minutes(30),
        DbType::PostgreSQL => Duration::minutes(60),
        DbType::MySQL => Duration::minutes(60),
        DbType::MongoDB => Duration::minutes(60),
        DbType::Redis => Duration::minutes(60),
        DbType::SQLite => Duration::hours(24),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Active,
    Idle,
    Expired,
}

#[derive(Debug, Clone)]
pub enum SessionEvent {
    /// Session will expire in N seconds — warn the user
    ExpiryWarning {
        session_id: Uuid,
        connection_id: Uuid,
        remaining_secs: i64,
    },
    /// Session has expired
    Expired {
        session_id: Uuid,
        connection_id: Uuid,
    },
    /// Session was renewed by user activity
    Renewed {
        session_id: Uuid,
        connection_id: Uuid,
        expires_at: DateTime<Utc>,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct QuerySession {
    pub session_id: Uuid,
    pub connection_id: Uuid,
    pub user_id: Uuid,
    pub db_type: DbType,
    pub status: SessionStatus,
    pub created_at: DateTime<Utc>,
    pub last_active: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

impl QuerySession {
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    pub fn time_remaining_secs(&self) -> i64 {
        (self.expires_at - Utc::now()).num_seconds().max(0)
    }

    pub fn touch(&mut self) {
        let now = Utc::now();
        self.last_active = now;
        self.expires_at = now + ttl_for(&self.db_type);
        self.status = SessionStatus::Active;
    }
}

pub struct SessionRegistry {
    /// session_id → session
    sessions: DashMap<Uuid, QuerySession>,
    /// user_id → set of session_ids
    by_user: DashMap<Uuid, Vec<Uuid>>,
    /// Broadcast channel — WS layer subscribes to receive expiry events
    events: broadcast::Sender<SessionEvent>,
}

impl SessionRegistry {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(256);
        Self {
            sessions: DashMap::new(),
            by_user: DashMap::new(),
            events: tx,
        }
    }

    /// Subscribe to session lifecycle events
    pub fn subscribe(&self) -> broadcast::Receiver<SessionEvent> {
        self.events.subscribe()
    }

    /// Create a new session for a user + connection
    pub fn create(&self, connection_id: Uuid, user_id: Uuid, db_type: DbType) -> QuerySession {
        let now = Utc::now();
        let session_id = Uuid::new_v4();
        let expires_at = now + ttl_for(&db_type);

        let session = QuerySession {
            session_id,
            connection_id,
            user_id,
            db_type,
            status: SessionStatus::Active,
            created_at: now,
            last_active: now,
            expires_at,
        };

        self.sessions.insert(session_id, session.clone());
        self.by_user.entry(user_id).or_default().push(session_id);

        tracing::info!(
            session_id  = %session_id,
            connection_id = %connection_id,
            user_id     = %user_id,
            expires_at  = %expires_at,
            "session created"
        );

        session
    }

    /// Get session — returns None if expired or not found
    pub fn get(&self, session_id: Uuid) -> Option<QuerySession> {
        self.sessions.get(&session_id).map(|s| s.clone())
    }

    /// Get active session for a user + connection pair
    pub fn get_for_connection(&self, user_id: Uuid, connection_id: Uuid) -> Option<QuerySession> {
        let session_ids = self.by_user.get(&user_id)?;

        session_ids.iter().find_map(|sid| {
            self.sessions.get(sid).and_then(|s| {
                if s.connection_id == connection_id && !s.is_expired() {
                    Some(s.clone())
                } else {
                    None
                }
            })
        })
    }

    /// Touch session — extends TTL on every query execution
    pub fn touch(&self, session_id: Uuid) -> Option<DateTime<Utc>> {
        self.sessions.get_mut(&session_id).map(|mut s| {
            s.touch();
            let expires_at = s.expires_at;

            let _ = self.events.send(SessionEvent::Renewed {
                session_id,
                connection_id: s.connection_id,
                expires_at,
            });

            expires_at
        })
    }

    /// Explicitly expire a session (user disconnects / logout)
    pub fn expire(&self, session_id: Uuid) {
        if let Some(mut s) = self.sessions.get_mut(&session_id) {
            s.status = SessionStatus::Expired;
            s.expires_at = Utc::now();

            let _ = self.events.send(SessionEvent::Expired {
                session_id,
                connection_id: s.connection_id,
            });
        }
        self.sessions.remove(&session_id);
    }

    /// List all active sessions for a user
    pub fn list_for_user(&self, user_id: Uuid) -> Vec<QuerySession> {
        let Some(ids) = self.by_user.get(&user_id) else {
            return vec![];
        };

        ids.iter()
            .filter_map(|sid| {
                self.sessions.get(sid).and_then(|s| {
                    if !s.is_expired() {
                        Some(s.clone())
                    } else {
                        None
                    }
                })
            })
            .collect()
    }

    fn remove_expired(&self) {
        let expired: Vec<Uuid> = self
            .sessions
            .iter()
            .filter(|s| s.is_expired())
            .map(|s| s.session_id)
            .collect();

        for session_id in expired {
            if let Some((_, session)) = self.sessions.remove(&session_id) {
                tracing::info!(
                    session_id    = %session_id,
                    connection_id = %session.connection_id,
                    "session expired"
                );

                let _ = self.events.send(SessionEvent::Expired {
                    session_id,
                    connection_id: session.connection_id,
                });
            }
        }
    }
}
