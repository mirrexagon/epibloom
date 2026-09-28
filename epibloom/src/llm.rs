//! Container for active LLM requests.

use std::sync::Arc;

use futures::StreamExt;
use serde_json::json;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use tokio::select;
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;
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

        let cancellation_token = CancellationToken::new();
        let task_cancellation_token = cancellation_token.clone();

        tokio::spawn(async move {
            let response_result = select! {
                response_result = anthropic_client.generate(request) => response_result.map_err(From::from),
                _ = task_cancellation_token.cancelled() => {
                    trace.error = Some("Generation was cancelled".to_owned());
                    if let Err(err) = trace.sync_state_object().await {
                        log::error!(
                            "Failed to update trace (state object '{}') after cancellation: {err}",
                            trace.state_object.name
                        ),
                    }
                    Err(Error::Cancelled)
                },
            };

            let end_time = OffsetDateTime::now_utc();

            trace.end_time = Some(end_time);
            match &response_result {
                Ok(response) => trace.response = Some(response.clone()),
                Err(err) => trace.error = Some(err.to_string()),
            }

            match trace.sync_state_object().await {
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
            cancellation_token,
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

        let cancellation_token = CancellationToken::new();
        let task_cancellation_token = cancellation_token.clone();

        tokio::spawn(async move {
            let stream = anthropic_client.generate_streaming(request);
            futures::pin_mut!(stream);

            while let Some(result) = select! {
                item = stream.next() => item.map(|r| r.map_err(From::from)),
                _ = task_cancellation_token.cancelled() => {
                    trace.error = Some("Generation was cancelled".to_owned());
                    if let Err(err) = trace.sync_state_object().await {
                        log::error!(
                            "Couldn't update trace (state object '{}') on cancellation: {err}",
                            trace.state_object.name
                        );
                    }
                    Some(Err(Error::Cancelled))
                },
            } {
                log::trace!("generate_streaming result: {result:#?}");

                match &result {
                    Ok(event) => match event {
                        messages::response::MessageStreamEvent::MessageStart { message } => {
                            trace.response = Some(message.clone());
                        }
                        e => match &mut trace.response {
                            Some(message) => message.accumulate(e),
                            None => {
                                log::error!(
                                    "Anthropic streaming request (state object '{}') received a non-MessageStart event before a MessageStart event: {e:?}",
                                    trace.state_object.name
                                )
                            }
                        },
                    },
                    Err(err) => {
                        trace.error = Some(err.to_string());

                        // When we fall out of the loop, we and set the end time
                        // and save the trace.
                        break;
                    }
                }

                if let Err(err) = trace.sync_state_object().await {
                    log::error!(
                        "Failed to update trace (state object '{}') with non-streaming response: {err}",
                        trace.state_object.name
                    );
                }

                if let Err(err) = tx.send(result).await {
                    log::warn!("Failed to send StreamingEvent into channel: {err:?}");
                }
            }

            trace.end_time = Some(OffsetDateTime::now_utc());
            if let Err(err) = trace.sync_state_object().await {
                log::error!(
                    "Couldn't update trace (state object '{}') at streaming end: {err}",
                    trace.state_object.name
                );
            }
        });

        Ok(AnthropicGenerateStreamingHandle {
            trace_state_object_name,
            receiver: Some(rx),
            cancellation_token,
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

            start_time: OffsetDateTime::now_utc(),
            end_time: None,
            request: request.clone(),
            response: None,
            error: None,

            state_object: self.state_store.get_or_create(&state_object_name).await?,
        };

        trace.sync_state_object().await?;

        Ok(trace)
    }
}

// TODO: Being able to cancel from another script that doesn't have the Lua
// handle, getting by trace UUID.
pub struct AnthropicGenerateHandle {
    trace_state_object_name: String,

    receiver: Option<oneshot::Receiver<Result<messages::Message, Error>>>,
    cancellation_token: CancellationToken,

    /// The final full response.
    response: Option<Result<messages::Message, Error>>,
}

impl AnthropicGenerateHandle {
    pub fn get_trace_state_object_name(&self) -> &str {
        &self.trace_state_object_name
    }

    pub async fn get_response(&mut self) -> Result<&messages::Message, &Error> {
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

    /// Cancel the request.
    pub fn cancel(&self) {
        self.cancellation_token.cancel()
    }
}

pub struct AnthropicGenerateStreamingHandle {
    trace_state_object_name: String,

    receiver: Option<mpsc::Receiver<Result<messages::response::MessageStreamEvent, Error>>>,
    cancellation_token: CancellationToken,

    /// The final full response up to either when the generation completed
    /// successfully or an error occurred.
    response: Option<messages::Message>,

    // The error that occurred, if one did.
    error: Option<Error>,
}

impl AnthropicGenerateStreamingHandle {
    pub fn get_trace_state_object_name(&self) -> &str {
        &self.trace_state_object_name
    }

    pub async fn get_next_event(
        &mut self,
    ) -> Option<Result<messages::response::MessageStreamEvent, &Error>> {
        let event = match self.receiver {
            Some(ref mut rx) => rx.recv().await,

            // Channel has been dropped because it was done.
            None => return None,
        };

        match event {
            Some(event) => match event {
                Ok(event) => {
                    match &mut self.response {
                        Some(response) => {
                            // TODO: Don't duplicate the accumulation that happens in the streaming task.
                            response.accumulate(&event);
                        }

                        None => {
                            if let messages::response::MessageStreamEvent::MessageStart {
                                ref message,
                            } = event
                            {
                                self.response = Some(message.clone());
                            } else {
                                log::error!(
                                    "AnthropicGenerateStreamingHandle (state object '{}') did not receive a MessageStart event as the first event.",
                                    self.trace_state_object_name
                                );
                            }
                        }
                    }

                    Some(Ok(event))
                }
                Err(err) => {
                    self.error = Some(err);
                    Some(Err(self.error.as_ref().unwrap()))
                }
            },

            // Channel is done.
            None => {
                self.receiver.take();
                None
            }
        }
    }

    /// Wait for the request to be complete and return the final result.
    pub async fn get_response(&mut self) -> (Option<&messages::Message>, Option<&Error>) {
        // Drain all events until the generation is done.
        while self.get_next_event().await.is_some() {
            // Let `get_next_event()` process any normal events and update `self.response`.
        }

        (self.response.as_ref(), self.error.as_ref())
    }

    /// Cancel the request.
    pub fn cancel(&self) {
        self.cancellation_token.cancel()
    }
}

#[derive(Debug)]
pub struct AnthropicTrace {
    id: Uuid,

    start_time: OffsetDateTime,
    end_time: Option<OffsetDateTime>,
    request: messages::request::MessagesRequest,
    response: Option<messages::Message>,
    error: Option<String>,

    state_object: Arc<StateObject>,
}

impl AnthropicTrace {
    async fn sync_state_object(&self) -> Result<(), Error> {
        // TODO: In the response, don't update the entire text string each time.
        // Only send the change.
        let data = json!({
            "start_time": self.start_time.format(&Rfc3339)?,
            "end_time": self.end_time.map(|t| t.format(&Rfc3339)).transpose()?,
            "request": &self.request,
            "response": &self.response,
            "error": self.error.as_ref().map(|e| e.to_string()),
        });

        self.state_object
            .update(async |_| serde_json::to_value(&data))
            .await?;

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Generation was cancelled")]
    Cancelled,
    #[error("I/O error: {0}")]
    Io(#[from] tokio::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Time formatting error: {0}")]
    TimeFormatting(#[from] time::error::Format),
    #[error("Anthropic error: {0}")]
    Anthropic(#[from] anthropic::Error),
    #[error("State object error: {0}")]
    StateObject(#[from] state::Error),
}
