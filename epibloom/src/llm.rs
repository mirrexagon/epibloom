//! Container for active LLM requests.
//!
//! TODO: Each request creates a state object `llm-trace:<uuid>` that is updated with the contents of the response, for monitoring.

use std::sync::Arc;

use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::{
    backend::anthropic::{self, messages, models, AnthropicClient},
    state::StateStore,
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

    /// Start an LLM generation.
    ///
    /// TODO: Creates the `llm-trace:<uuid>` object and creates or updates the
    /// `llm-trace-index` state object, spawns a task that streams chunks into
    /// the trace state object, and returns the trace UUID string plus an
    /// [`mpsc::Receiver`] that delivers the same chunks to the caller.
    ///
    /// TODO: Should this just return a stream that wraps the request stream,
    /// modifying the trace state object before passing on the event? Might mean
    /// it can be cancelled by dropping.
    pub async fn anthropic_generate(
        &self,
        request: messages::request::MessagesRequest,
    ) -> Result<messages::response::Message, anthropic::Error> {
        self.anthropic_client.generate(request).await
    }

    pub async fn anthropic_generate_streaming(
        &self,
        request: messages::request::MessagesRequest,
    ) -> mpsc::Receiver<AnthropicMessagesEvent> {
        let backend = Arc::clone(&self.anthropic_client);
        let (tx, rx) = mpsc::channel(1024);
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
                            log::error!("Failed to send StreamingEvent into channel: {err:?}");
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

        rx
    }
}
