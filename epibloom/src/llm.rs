//! Container for active LLM requests.
//!
//! TODO: Each request creates a state object `llm-trace:<uuid>` that is updated with the contents of the response, for monitoring.

use std::future::Future;
use std::sync::Arc;

use futures::{FutureExt, StreamExt};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use crate::{
    backend::anthropic::{self, messages, AnthropicClient},
    state::{self, StateObject, StateStore},
};

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

    pub async fn anthropic_generate(
        &self,
        request: messages::request::MessagesRequest,
    ) -> Result<oneshot::Receiver<Result<messages::Message, Error>>, Error> {
        let trace_object = self.new_anthropic_llm_trace_state_object(&request).await?;

        let anthropic_client = Arc::clone(&self.anthropic_client);
        let (tx, rx) = oneshot::channel();

        tokio::spawn(async move {
            // TODO: Handle prefill, if the last message is an assistant message,
            // stick the response on the end of that.

            let response_result = anthropic_client.generate(request).await;
            let end_time = OffsetDateTime::now_utc();

            match trace_object
                .update(async |value| {
                    let mut trace: AnthropicLlmTrace = serde_json::from_value(value)?;

                    trace.end_time = Some(end_time);

                    match &response_result {
                        Ok(response) => trace.trace.messages.push(response.clone()),
                        Err(err) => trace.error = Some(err.to_string()),
                    }

                    serde_json::to_value(trace)
                })
                .await
            {
                Ok(_) => {}
                Err(err) => log::error!(
                    "Failed to update trace '{}' with response: {err}",
                    &trace_object.name
                ),
            }

            let _ = tx.send(response_result.map_err(Error::from));
        });

        Ok(rx)
    }

    // TODO: Should this just return a stream that wraps the request stream,
    // modifying the trace state object before passing on the event? Might mean
    // it can be cancelled by dropping. As long as we can buffer events for Lua
    // to get at its own pace, the channel currently works as the queue.
    pub async fn anthropic_generate_streaming(
        &self,
        request: messages::request::MessagesRequest,
    ) -> Result<mpsc::Receiver<Result<messages::response::MessageStreamEvent, Error>>, Error> {
        let trace_object = self.new_anthropic_llm_trace_state_object(&request).await?;

        let anthropic_client = Arc::clone(&self.anthropic_client);
        let (tx, rx) = mpsc::channel(1024);

        tokio::spawn(async move {
            let stream = anthropic_client.generate_streaming(request);
            futures::pin_mut!(stream);

            while let Some(result) = stream.next().await {
                if let Err(err) = tx.send(result.map_err(Error::from)).await {
                    log::warn!("Failed to send StreamingEvent into channel: {err:?}");
                }

                // TODO: Update the trace object.
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
