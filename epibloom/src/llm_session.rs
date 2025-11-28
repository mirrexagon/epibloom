use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tokio::sync::{broadcast, RwLock};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::backends::{self, Backend};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LlmSessionMetadata {
    pub id: Uuid,
    pub name: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub modified: OffsetDateTime,
}

pub struct LlmSession {
    pub metadata: LlmSessionMetadata,
    pub chat: backends::Chat,
    pub current_assistant_message: String,
    pub broadcast: broadcast::Sender<backends::GenerateChunk>,
}

impl LlmSession {
    pub fn new(metadata: LlmSessionMetadata, chat: backends::Chat) -> Arc<Self> {
        // 1024 capacity allows late subscribers to catch up on recent messages
        // if they subscribe while chunks are being broadcast
        let (broadcast, _) = broadcast::channel(1024);

        Arc::new(Self {
            metadata,
            chat: RwLock::new(chat),
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

/// Creates a new LLM session, adds it to the store, and starts generation in the background.
pub async fn create_and_start_session(
    name: String,
    chat: backends::Chat,
    backend: BackendChoice,
    anthropic_api: AnthropicApi,
    featherless_api: FeatherlessApi,
    store: &LlmSessionStore,
) -> Arc<LlmSessionState> {
    let session_id = Uuid::new_v4();
    let now = OffsetDateTime::now_utc();

    let metadata = LlmSessionMetadata {
        id: session_id,
        name,
        created: now,
        modified: now,
    };

    let session = LlmSessionState::new(metadata, chat.clone());
    store.add_session(session.clone()).await;

    // Spawn background task to generate
    let session_clone = session.clone();
    tokio::spawn(async move {
        run_generation(session_clone, backend, chat, anthropic_api, featherless_api).await;
    });

    session
}

async fn run_generation(
    session: Arc<LlmSessionState>,
    backend: BackendChoice,
    chat: backends::Chat,
    anthropic_api: AnthropicApi,
    featherless_api: FeatherlessApi,
) {
    // Update status to Generating
    *session.status.write().await = SessionStatus::Generating;

    // Create the stream based on backend choice
    let result: Result<String, Box<dyn std::error::Error + Send + Sync>> = match backend {
        BackendChoice::Claude => {
            let config = backends::anthropic::Config {
                max_tokens: 2048,
                thinking_budget: Some(1024),
                ..Default::default()
            };
            let stream = anthropic_api.generate(&config, &chat);
            process_generation_stream(session.clone(), stream).await
        }
        BackendChoice::Featherless => {
            let config = backends::featherless::Config::default();
            let stream = featherless_api.generate(&config, &chat);
            process_generation_stream(session.clone(), stream).await
        }
    };

    // Update status based on result
    match result {
        Ok(assistant_message) => {
            // Add the completed assistant message to the chat
            let mut chat = session.chat.write().await;
            chat.messages.push(backends::Message {
                role: backends::Role::Assistant,
                text: assistant_message,
            });
            drop(chat);

            *session.status.write().await = SessionStatus::Idle;
        }
        Err(err) => {
            *session.status.write().await = SessionStatus::Error(err.to_string());
        }
    }
}

async fn process_generation_stream<S, E>(
    session: Arc<LlmSessionState>,
    stream: S,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>>
where
    S: futures::Stream<Item = Result<backends::GenerateChunk, E>>,
    E: std::error::Error + Send + Sync + 'static,
{
    use futures::stream::StreamExt;

    futures::pin_mut!(stream);

    let mut accumulated_message = String::new();

    while let Some(result) = stream.next().await {
        match result {
            Ok(chunk) => {
                // Accumulate message content
                if let backends::GenerateChunk::MessageDelta(ref delta) = chunk {
                    accumulated_message.push_str(delta);
                }

                // Add to history
                session.history.write().await.push(chunk.clone());

                // Broadcast to subscribers (ignore if no receivers)
                let _ = session.broadcast.send(chunk);
            }
            Err(err) => {
                return Err(Box::new(err));
            }
        }
    }

    Ok(accumulated_message)
}
