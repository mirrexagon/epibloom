//! Container for active LLM requests.

use std::sync::Arc;

use futures::StreamExt;
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
    ) -> Result<AnthropicGenerateHandle, Error> {
        let mut trace = self.new_anthropic_llm_trace_state_object(&request).await?;
        let trace_state_object_name = trace.state_object.name.clone();

        let anthropic_client = Arc::clone(&self.anthropic_client);
        let (tx, rx) = oneshot::channel();

        tokio::spawn(async move {
            let response_result = anthropic_client.generate(request).await;
            let end_time = OffsetDateTime::now_utc();

            trace.content.end_time = Some(end_time);
            // TODO: Handle prefill, if the last message is an assistant message,
            // stick the response on the end of that.
            match &response_result {
                Ok(response) => trace.content.trace.messages.push(response.clone()),
                Err(err) => trace.content.error = Some(err.to_string()),
            }

            match trace.update_state_object_from_content().await {
                Ok(_) => {}
                Err(err) => log::error!(
                    "Failed to update trace (state object '{}') with non-streaming response: {err}",
                    trace.state_object.name
                ),
            }

            if tx.send(response_result).is_err() {
                log::warn!(
                    "llm::anthropic_generate trace (state object '{}'): tried to send non-streaming response but the channel has already been closed",
                    trace.state_object.name
                );
            }
        });

        Ok(AnthropicGenerateHandle {
            trace_state_object_name,
            receiver: Some(rx),
            response: None,
        })
    }

    pub async fn anthropic_generate_streaming(
        &self,
        request: messages::request::MessagesRequest,
    ) -> Result<AnthropicGenerateStreamingHandle, Error> {
        let mut trace = self.new_anthropic_llm_trace_state_object(&request).await?;
        let trace_state_object_name = trace.state_object.name.clone();

        let anthropic_client = Arc::clone(&self.anthropic_client);
        let (tx, rx) = mpsc::channel(1024);

        tokio::spawn(async move {
            let stream = anthropic_client.generate_streaming(request);
            futures::pin_mut!(stream);

            while let Some(result) = stream.next().await {
                match &result {
                    Ok(event) => {
                        // TODO: Update the trace.

                        // TODO: Handle prefill, if the last message is an assistant message,
                        // stick the response on the end of that.
                    }
                    Err(err) => {
                        trace.content.error = Some(err.to_string());

                        // The error should be the last thing in the channel, so
                        // when we go around again, we we fall out of the loop
                        // and set the end time and save the trace then.
                    }
                }

                if let Err(err) = tx.send(result).await {
                    log::warn!("Failed to send StreamingEvent into channel: {err:?}");
                }
            }

            trace.content.end_time = Some(OffsetDateTime::now_utc());
            if let Err(err) = trace.update_state_object_from_content().await {
                log::error!(
                    "Couldn't update trace (state object '{}') at streaming end: {err}",
                    trace.state_object.name
                );
            }
        });

        Ok(AnthropicGenerateStreamingHandle {
            trace_state_object_name,
            receiver: Some(rx),
            response: None,
            error: None,
        })
    }

    async fn new_anthropic_llm_trace_state_object(
        &self,
        request: &messages::request::MessagesRequest,
    ) -> Result<AnthropicTrace, Error> {
        let trace_id = Uuid::now_v7();
        let state_object_name = format!("llm-trace-anthropic:{}:persist", trace_id);

        let trace = AnthropicTrace {
            id: trace_id,
            content: AnthropicTraceContents::from(request.clone()),
            state_object: self.state_store.get_or_create(&state_object_name).await?,
        };

        trace.update_state_object_from_content().await?;

        Ok(trace)
    }
}

pub struct AnthropicGenerateHandle {
    trace_state_object_name: String,

    receiver: Option<oneshot::Receiver<Result<messages::Message, anthropic::Error>>>,

    /// The final full response.
    response: Option<Result<messages::Message, anthropic::Error>>,
}

impl AnthropicGenerateHandle {
    pub fn get_trace_state_object_name(&self) -> &str {
        &self.trace_state_object_name
    }

    pub async fn get_response(&mut self) -> Result<&messages::Message, &anthropic::Error> {
        match self.receiver.take() {
            Some(receiver) => {
                // `receiver.await` will only fail if the sender is dropped.
                // If this happens, we've done something very wrong.
                self.response = Some(
                    receiver
                        .await
                        .expect("AnthropicGenerateHandle channel sender has been dropped"),
                );
                self.response.as_ref().unwrap().as_ref()
            }
            None => self.response.as_ref().unwrap().as_ref(),
        }
    }
}

pub struct AnthropicGenerateStreamingHandle {
    trace_state_object_name: String,

    receiver:
        Option<mpsc::Receiver<Result<messages::response::MessageStreamEvent, anthropic::Error>>>,

    /// The final full response up to either when the generation completed
    /// successfully or an error occurred.
    response: Option<messages::Message>,

    // The error that occurred, if one did.
    error: Option<anthropic::Error>,
}

impl AnthropicGenerateStreamingHandle {
    pub fn get_trace_state_object_name(&self) -> &str {
        &self.trace_state_object_name
    }

    pub async fn get_next_event(
        &mut self,
    ) -> Option<Result<messages::response::MessageStreamEvent, &anthropic::Error>> {
        let event = match self.receiver {
            Some(ref mut rx) => rx.recv().await,

            // Channel has been dropped because it was done.
            None => return None,
        };

        match event {
            Some(event) => {
                match event {
                    Ok(event) => {
                        // TODO: Update self.response

                        Some(Ok(event))
                    }
                    Err(err) => {
                        self.error = Some(err);
                        Some(Err(self.error.as_ref().unwrap()))
                    }
                }
            }

            // Channel is done.
            None => {
                self.receiver.take();
                None
            }
        }
    }

    /// Wait for the request to be complete and return the final result.
    pub async fn get_response(
        &mut self,
    ) -> (Option<&messages::Message>, Option<&anthropic::Error>) {
        // Drain all events until the generation is done.
        while self.get_next_event().await.is_some() {
            // Let `get_next_event()` process any normal events and update `self.response`.
        }

        (self.response.as_ref(), self.error.as_ref())
    }
}

#[derive(Debug)]
pub struct AnthropicTrace {
    id: Uuid,
    content: AnthropicTraceContents,
    state_object: Arc<StateObject>,
}

impl AnthropicTrace {
    async fn update_state_object_from_content(&self) -> Result<(), Error> {
        self.state_object
            .update(async |_| serde_json::to_value(&self.content))
            .await?;

        Ok(())
    }
}

/// Trace state object contents.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AnthropicTraceContents {
    #[serde(with = "time::serde::rfc3339")]
    start_time: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    end_time: Option<OffsetDateTime>,

    trace: messages::request::MessagesRequest,
    error: Option<String>,
}

impl From<messages::request::MessagesRequest> for AnthropicTraceContents {
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
