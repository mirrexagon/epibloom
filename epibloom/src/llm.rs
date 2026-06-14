//! Container for active LLM requests.
//!
//! TODO: Each request creates a state object `llm-trace:<uuid>` that is updated with the contents of the response, for monitoring.

use std::future::Future;
use std::sync::Arc;

use futures::StreamExt;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::{
    backend::anthropic::{self, messages, AnthropicClient},
    state::{self, StateObject, StateStore},
};

#[derive(Debug, Serialize, Deserialize)]
pub enum AnthropicMessagesEvent {
    MessageStreamEvent(messages::response::MessageStreamEvent),
    Error(String),
}

pub struct LlmStore {
    pub anthropic_client: Arc<AnthropicClient>,
    state_store: Arc<StateStore>,
}

impl LlmStore {
    pub fn new(state_store: Arc<StateStore>) -> Self {
        Self {
            anthropic_client: Arc::new(
                AnthropicClient::new_from_env().expect("Failed to create Anthropic API client."),
            ),
            state_store,
        }
    }

    pub fn anthropic_generate(
        &self,
        request: messages::request::MessagesRequest,
    ) -> Result<impl Future<Output = Result<messages::Message, Error>>, Error> {
        self.anthropic_generate_internal(request)
    }

    async fn anthropic_generate_internal(
        &self,
        request: messages::request::MessagesRequest,
    ) -> Result<messages::Message, Error> {
        let trace_object = self.new_anthropic_llm_trace_state_object(&request).await?;
        log::info!("Starting Anthropic generation: {}", &trace_object.name);

        let response_result = self.anthropic_client.generate(request).await;
        let end_time = OffsetDateTime::now_utc();

        // TODO: Handle prefill, if the last message is an assistant message,
        // stick the response on the end of that.

        trace_object
            .update(async |value| {
                let mut trace: AnthropicLlmTrace = serde_json::from_value(value)?;

                trace.end_time = Some(end_time);

                match &response_result {
                    Ok(response) => trace.trace.messages.push(response.clone()),
                    Err(err) => trace.error = Some(err.to_string()),
                }

                serde_json::to_value(trace)
            })
            .await?;

        Ok(response_result?)
    }

    // TODO: Should this just return a stream that wraps the request stream,
    // modifying the trace state object before passing on the event? Might mean
    // it can be cancelled by dropping. As long as we can buffer events for Lua
    // to get at its own pace, the channel currently works as the queue.
    pub async fn anthropic_generate_streaming(
        &self,
        request: messages::request::MessagesRequest,
    ) -> Result<mpsc::Receiver<AnthropicMessagesEvent>, Error> {
        let backend = Arc::clone(&self.anthropic_client);
        let (tx, rx) = mpsc::channel(1024);

        let trace_object = self.new_anthropic_llm_trace_state_object(&request).await?;

        let stream = backend.generate_streaming(request);

        tokio::spawn(async move {
            futures::pin_mut!(stream);

            while let Some(result) = stream.next().await {
                match result {
                    Ok(event) => {
                        if let Err(err) = tx
                            .send(AnthropicMessagesEvent::MessageStreamEvent(event))
                            .await
                        {
                            log::warn!("Failed to send StreamingEvent into channel: {err:?}");
                            return;
                        }
                    }

                    Err(err) => {
                        log::error!("Error in LLM generation stream: {err:?}");
                        let _ = tx
                            .send(AnthropicMessagesEvent::Error(err.to_string()))
                            .await;
                        return;
                    }
                }
            }
        });

        Ok(rx)
    }

    async fn new_anthropic_llm_trace_state_object(
        &self,
        request: &messages::request::MessagesRequest,
    ) -> Result<Arc<StateObject>, state::Error> {
        let trace = AnthropicLlmTrace::from(request.clone());

        let name = format!("llm-trace-anthropic:{}:persist", Uuid::now_v7());
        let state_object = self.state_store.get_or_create(&name).await?;

        state_object
            .update(async |_| serde_json::to_value(trace))
            .await?;

        Ok(state_object)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AnthropicLlmTrace {
    #[serde(with = "time::serde::rfc3339")]
    start_time: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    end_time: Option<OffsetDateTime>,

    trace: messages::request::MessagesRequest,
    error: Option<String>,
}

impl From<messages::request::MessagesRequest> for AnthropicLlmTrace {
    fn from(request: messages::request::MessagesRequest) -> Self {
        Self {
            start_time: OffsetDateTime::now_utc(),
            end_time: None,
            trace: request,
            error: None,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] tokio::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Anthropic error: {0}")]
    Anthropic(#[from] anthropic::Error),
    #[error("State object error: {0}")]
    StateObject(#[from] state::Error),
}
