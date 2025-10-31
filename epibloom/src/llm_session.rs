use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use time::macros::format_description;
use time::OffsetDateTime;
use tokio::sync::{broadcast, RwLock};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::backends;
use crate::util;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LlmSessionMetadata {
    pub id: Uuid,
    pub name: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub modified: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LlmSession {
    #[serde(flatten)]
    pub metadata: LlmSessionMetadata,
    pub chat: backends::Chat,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    Idle,
    Generating,
    Error(String),
}

/// Runtime state for an active LLM session
pub struct LlmSessionState {
    pub metadata: LlmSessionMetadata,
    pub status: RwLock<SessionStatus>,
    pub history: RwLock<Vec<backends::GenerateChunk>>,
    pub broadcast: broadcast::Sender<backends::GenerateChunk>,
}

impl LlmSessionState {
    pub fn new(metadata: LlmSessionMetadata) -> Arc<Self> {
        // 1024 capacity allows late subscribers to catch up on recent messages
        // if they subscribe while chunks are being broadcast
        let (broadcast, _) = broadcast::channel(1024);

        Arc::new(Self {
            metadata,
            status: RwLock::new(SessionStatus::Generating),
            history: RwLock::new(Vec::new()),
            broadcast,
        })
    }
}

/// In-memory store for active LLM sessions
#[derive(Clone)]
pub struct LlmSessionStore {
    sessions: Arc<RwLock<HashMap<Uuid, Arc<LlmSessionState>>>>,
}

impl LlmSessionStore {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add a new session to the store
    pub async fn add_session(&self, session: Arc<LlmSessionState>) {
        let mut sessions = self.sessions.write().await;
        sessions.insert(session.metadata.id, session);
    }

    /// Get a session by ID
    pub async fn get_session(&self, id: Uuid) -> Option<Arc<LlmSessionState>> {
        let sessions = self.sessions.read().await;
        sessions.get(&id).cloned()
    }

    /// Get all session metadata
    pub async fn list_sessions(&self) -> Vec<LlmSessionMetadata> {
        let sessions = self.sessions.read().await;
        sessions
            .values()
            .map(|state| state.metadata.clone())
            .collect()
    }

    /// Remove a session from the store
    pub async fn remove_session(&self, id: Uuid) -> Option<Arc<LlmSessionState>> {
        let mut sessions = self.sessions.write().await;
        sessions.remove(&id)
    }
}
