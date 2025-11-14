use rocket::{
    get, post,
    response::stream::{Event, EventStream},
    serde::json::Json,
    State,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::ApiErrorResponse;
use crate::backends::{self, anthropic::AnthropicApi, featherless::FeatherlessApi};
use crate::llm_session::{BackendChoice, LlmSessionMetadata, LlmSessionStore, SessionStatus};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct GetLlmSessionsResponse {
    llm_sessions: Vec<LlmSessionMetadata>,
}

/// Gets a list of all LLM sessions (metadata only).
#[get("/")]
pub async fn get_llm_sessions(
    store: &State<LlmSessionStore>,
) -> Result<Json<GetLlmSessionsResponse>, ApiErrorResponse> {
    let llm_sessions = store.list_sessions().await;
    Ok(Json(GetLlmSessionsResponse { llm_sessions }))
}

/// Creates a test LLM session with hard-coded chat and starts generation.
#[post("/test")]
pub async fn create_test_session(
    store: &State<LlmSessionStore>,
    anthropic_api: &State<AnthropicApi>,
    featherless_api: &State<FeatherlessApi>,
) -> Result<Json<LlmSessionMetadata>, ApiErrorResponse> {
    // Create a hard-coded test chat
    let chat = backends::Chat {
        messages: vec![backends::Message {
            role: backends::Role::User,
            text: "What is the capital of France?".to_string(),
        }],
    };

    let session = crate::llm_session::create_and_start_session(
        "Test Session".to_string(),
        chat,
        BackendChoice::Claude,
        anthropic_api.inner().clone(),
        featherless_api.inner().clone(),
        store.inner(),
    )
    .await;

    Ok(Json(session.metadata.clone()))
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateSessionRequest {
    pub name: String,
    pub backend: BackendChoice,
    pub chat: backends::Chat,
}

/// Creates a new LLM session and starts generation in the background.
#[post("/", data = "<request>")]
pub async fn create_session(
    request: Json<CreateSessionRequest>,
    store: &State<LlmSessionStore>,
    anthropic_api: &State<AnthropicApi>,
    featherless_api: &State<FeatherlessApi>,
) -> Result<Json<LlmSessionMetadata>, ApiErrorResponse> {
    let session = crate::llm_session::create_and_start_session(
        request.name.clone(),
        request.chat.clone(),
        request.backend.clone(),
        anthropic_api.inner().clone(),
        featherless_api.inner().clone(),
        store.inner(),
    )
    .await;

    Ok(Json(session.metadata.clone()))
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    Status { status: SessionStatus },
    History { chat: backends::Chat },
    ThinkingDelta { delta: String },
    MessageDelta { delta: String },
}

/// Streams updates from an LLM session via SSE.
/// First sends chat history and status, then streams live updates.
#[get("/<id>")]
pub async fn stream_session(
    id: Uuid,
    store: &State<LlmSessionStore>,
) -> Result<EventStream![], ApiErrorResponse> {
    // Get the session
    let session = store
        .get_session(id)
        .await
        .ok_or(ApiErrorResponse::NotFound(format!(
            "Session with id {} not found",
            id
        )))?;

    Ok(EventStream! {
        // Subscribe to live updates BEFORE reading history to avoid race condition
        let mut rx = session.broadcast.subscribe();

        // Send current status
        let status = session.status.read().await.clone();
        yield Event::json(&StreamEvent::Status { status: status.clone() });

        // Send chat history as a single event
        let chat = session.chat.read().await.clone();
        yield Event::json(&StreamEvent::History { chat });

        // Send all streaming chunks (thinking and partial messages)
        let history = session.history.read().await;
        for chunk in history.iter() {
            let event = match chunk {
                backends::GenerateChunk::ThinkingDelta(delta) => {
                    StreamEvent::ThinkingDelta { delta: delta.clone() }
                }
                backends::GenerateChunk::MessageDelta(delta) => {
                    StreamEvent::MessageDelta { delta: delta.clone() }
                }
            };
            yield Event::json(&event);
        }
        drop(history); // Release the read lock

        // Check if generation is still active after sending history
        let is_generating = matches!(status, SessionStatus::Generating);

        if !is_generating {
            // Generation already completed, we're done
            return;
        }

        // Stream live updates with timeout to check status periodically
        loop {
            let timeout_duration = std::time::Duration::from_secs(30);
            match tokio::time::timeout(timeout_duration, rx.recv()).await {
                Ok(Ok(chunk)) => {
                    let event = match chunk {
                        backends::GenerateChunk::ThinkingDelta(delta) => {
                            StreamEvent::ThinkingDelta { delta }
                        }
                        backends::GenerateChunk::MessageDelta(delta) => {
                            StreamEvent::MessageDelta { delta }
                        }
                    };
                    yield Event::json(&event);
                }
                Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => {
                    // Subscriber fell behind, some messages were dropped
                    // Continue receiving
                    continue;
                }
                Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => {
                    // Channel closed (shouldn't happen but handle it)
                    break;
                }
                Err(_) => {
                    // Timeout - check if generation is still active
                    let current_status = session.status.read().await.clone();
                    if !matches!(current_status, SessionStatus::Generating) {
                        // Generation completed, send final status and exit
                        yield Event::json(&StreamEvent::Status { status: current_status });
                        break;
                    }
                    // Still generating, keep waiting
                    continue;
                }
            }
        }
    })
}
