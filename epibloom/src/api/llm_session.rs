use futures::stream::StreamExt;
use rocket::{
    get, post,
    response::stream::{Event, EventStream},
    serde::json::Json,
    State,
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::Uuid;

use super::ApiErrorResponse;
use crate::backends::{self, anthropic::AnthropicApi, featherless::FeatherlessApi, Backend};
use crate::llm_session::{LlmSessionMetadata, LlmSessionState, LlmSessionStore, SessionStatus};

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

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum BackendChoice {
    Featherless,
    Claude,
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
    let session_id = Uuid::new_v4();
    let now = OffsetDateTime::now_utc();

    let metadata = LlmSessionMetadata {
        id: session_id,
        name: request.name.clone(),
        created: now,
        modified: now,
    };

    let session = LlmSessionState::new(metadata.clone());
    store.add_session(session.clone()).await;

    // Spawn background task to generate
    let chat = request.chat.clone();
    let backend = request.backend.clone();
    let anthropic_api = anthropic_api.inner().clone();
    let featherless_api = featherless_api.inner().clone();

    tokio::spawn(async move {
        run_generation(session, backend, chat, anthropic_api, featherless_api).await;
    });

    Ok(Json(metadata))
}

async fn run_generation(
    session: std::sync::Arc<LlmSessionState>,
    backend: BackendChoice,
    chat: backends::Chat,
    anthropic_api: AnthropicApi,
    featherless_api: FeatherlessApi,
) {
    // Update status to Generating
    *session.status.write().await = SessionStatus::Generating;

    // Create the stream based on backend choice
    let result: Result<(), Box<dyn std::error::Error + Send + Sync>> = match backend {
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
        Ok(()) => {
            *session.status.write().await = SessionStatus::Idle;
        }
        Err(err) => {
            *session.status.write().await = SessionStatus::Error(err.to_string());
        }
    }
}

async fn process_generation_stream<S, E>(
    session: std::sync::Arc<LlmSessionState>,
    stream: S,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    S: futures::Stream<Item = Result<backends::GenerateChunk, E>>,
    E: std::error::Error + Send + Sync + 'static,
{
    futures::pin_mut!(stream);

    while let Some(result) = stream.next().await {
        match result {
            Ok(chunk) => {
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

    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    Status { status: SessionStatus },
    ThinkingDelta { delta: String },
    MessageDelta { delta: String },
}

/// Streams updates from an LLM session via SSE.
/// First sends all historical chunks, then streams live updates.
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

        // Send all historical chunks
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
