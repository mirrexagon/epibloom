use std::sync::Arc;

use async_stream::stream;
use futures::Stream;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, Notify};
use utoipa::ToSchema;

use crate::backend::GenerateChunk;

use super::conversation::{Message, Role, Trace};
use super::Error;

// TODO: Use JSON Patch of an entire trace instead of custom events?
// Implement traces in terms of StateObjects and have backends and backened
// reuqests exposed directly to Lua where scripts can feed chunks into state
// objects that then get transmitted to the clients?
// Or have traces in Rust still, exposed to Lua in the same way, but use the state objects to transmit to clients.
//
// Maybe a bit crazy. Streaming message generation would mean sending the whole message string across with each update
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TraceEvent {
    MessageStart { position: usize, role: Role },
    ThinkingDelta { delta: String },
    ContentDelta { delta: String },
    MessageEnd { position: usize },
    Error { message: String },
}

pub enum GenerationState {
    Idle,
    Generating {
        thinking_so_far: String,
        content_so_far: String,
        complete_notify: Arc<Notify>,
    },
}

impl GenerationState {
    fn is_generating(&self) -> bool {
        matches!(self, GenerationState::Generating { .. })
    }
}

pub struct Session {
    trace: Trace,
    state: GenerationState,
    event_broadcast: broadcast::Sender<TraceEvent>,
}

impl Session {
    pub fn new(trace: Trace) -> Self {
        let (broadcast, _) = broadcast::channel(1024);

        Session {
            trace,
            state: GenerationState::Idle,
            event_broadcast: broadcast,
        }
    }

    pub fn trace(&self) -> &Trace {
        &self.trace
    }

    pub fn generation_state(&self) -> &GenerationState {
        &self.state
    }

    pub fn get_message(&self, message_position: usize) -> Option<Message> {
        self.trace.messages.get(message_position).cloned()
    }

    pub fn get_message_count(&self) -> usize {
        self.trace.messages.len()
    }

    /// Provide no message position to append a message.
    pub async fn set_message(
        &mut self,
        message_position: Option<usize>,
        message: Message,
    ) -> Result<(), Error> {
        if message.thinking.is_some() && message.role != Role::Assistant {
            return Err(Error::ThinkingSuppliedForNonAssistantMessage);
        }

        if self.state.is_generating() {
            return Err(Error::CantModifyTraceWhileGenerating);
        }

        let now = time::OffsetDateTime::now_local().unwrap();
        let message_position = message_position.unwrap_or(self.trace.messages.len());

        match self.trace.messages.len() {
            len if message_position == len => {
                // Push new message on the end.
                self.trace.messages.push(message);
            }

            len if message_position < len => {
                self.trace.messages[message_position] = message;
            }

            _ => return Err(Error::MessageNotFound),
        }

        // Broadcast that the message was added or changed.
        let message = &self.trace.messages[message_position];

        self.trace.metadata.modified = now;
        super::persistence::save_to_directory(&self.trace).await?;

        let _ = self.event_broadcast.send(TraceEvent::MessageStart {
            position: message_position,
            role: message.role,
        });

        if let Some(ref thinking) = message.thinking {
            let _ = self.event_broadcast.send(TraceEvent::ThinkingDelta {
                delta: thinking.to_owned(),
            });
        }

        let _ = self.event_broadcast.send(TraceEvent::ContentDelta {
            delta: message.content.to_owned(),
        });

        let _ = self.event_broadcast.send(TraceEvent::MessageEnd {
            position: message_position,
        });

        Ok(())
    }

    /// Get a stream that yields events corresponding to existing messages, then any future message chunks.
    pub async fn get_trace_stream(&self) -> impl Stream<Item = TraceEvent> {
        let mut rx = self.event_broadcast.subscribe();
        let past_messages = self.trace.messages.clone();
        let current_thinking = match &self.state {
            GenerationState::Generating {
                thinking_so_far, ..
            } => Some(thinking_so_far.clone()),
            GenerationState::Idle => None,
        };
        let current_content = match &self.state {
            GenerationState::Generating { content_so_far, .. } => Some(content_so_far.clone()),
            GenerationState::Idle => None,
        };
        let is_generating = self.state.is_generating();

        stream! {
            // Send past messages.
            for (position, message) in past_messages.iter().enumerate() {
                yield TraceEvent::MessageStart { position, role: message.role };
                if let Some(ref thinking) = message.thinking {
                    yield TraceEvent::ThinkingDelta { delta: thinking.clone() };
                }
                yield TraceEvent::ContentDelta { delta: message.content.clone() };
                yield TraceEvent::MessageEnd { position };
            }

            // Send current in-progress message so far.
            if is_generating {
                yield TraceEvent::MessageStart {
                    position: past_messages.len(),
                    role: Role::Assistant,
                };

                if let Some(ref thinking) = current_thinking {
                    if !thinking.is_empty() {
                        yield TraceEvent::ThinkingDelta {
                            delta: thinking.clone(),
                        };
                    }
                }

                if let Some(ref content) = current_content {
                    if !content.is_empty() {
                        yield TraceEvent::ContentDelta {
                            delta: content.clone(),
                        };
                    }
                }
            }

            // Send any new events/messages as they come in.
            use tokio::sync::broadcast::error::RecvError;

            loop {
                match rx.recv().await {
                    Ok(event) => {
                        yield event;
                    }
                    Err(RecvError::Lagged(num_skipped_messages)) => {
                        log::error!(
                            "Lagged behind {} messages from trace stream",
                            num_skipped_messages
                        );
                    }
                    Err(RecvError::Closed) => break,
                }
            }
        }
    }

    /// Transition from Idle to Generating. Returns the current trace snapshot
    /// and a broadcast receiver for events.
    pub fn begin_generation(&mut self) -> Result<(Trace, broadcast::Receiver<TraceEvent>), Error> {
        if self.state.is_generating() {
            return Err(Error::CantModifyTraceWhileGenerating);
        }

        self.state = GenerationState::Generating {
            thinking_so_far: String::new(),
            content_so_far: String::new(),
            complete_notify: Arc::new(Notify::new()),
        };

        Ok((self.trace.clone(), self.event_broadcast.subscribe()))
    }

    /// Process a generation chunk, accumulating content and broadcasting events.
    pub fn process_chunk(&mut self, chunk: &GenerateChunk) {
        if let GenerationState::Generating {
            ref mut thinking_so_far,
            ref mut content_so_far,
            ..
        } = self.state
        {
            match chunk {
                GenerateChunk::ThinkingDelta(delta) => {
                    thinking_so_far.push_str(delta);
                    let _ = self.event_broadcast.send(TraceEvent::ThinkingDelta {
                        delta: delta.clone(),
                    });
                }
                GenerateChunk::MessageDelta(delta) => {
                    content_so_far.push_str(delta);
                    let _ = self.event_broadcast.send(TraceEvent::ContentDelta {
                        delta: delta.clone(),
                    });
                }
            }
        }
    }

    /// Broadcast an error event.
    pub fn broadcast_error(&self, message: String) {
        let _ = self.event_broadcast.send(TraceEvent::Error { message });
    }

    /// Broadcast a message start event.
    pub fn broadcast_message_start(&self, position: usize, role: Role) {
        let _ = self
            .event_broadcast
            .send(TraceEvent::MessageStart { position, role });
    }

    /// Broadcast a message end event.
    pub fn broadcast_message_end(&self, position: usize) {
        let _ = self
            .event_broadcast
            .send(TraceEvent::MessageEnd { position });
    }

    /// Finish generation: commit the accumulated message to the trace and
    /// transition back to Idle.
    pub async fn finish_generation(&mut self) {
        let now = time::OffsetDateTime::now_local().unwrap();

        let old_generation = std::mem::replace(&mut self.state, GenerationState::Idle);

        if let GenerationState::Generating {
            thinking_so_far,
            content_so_far,
            complete_notify,
        } = old_generation
        {
            self.trace.messages.push(Message {
                role: Role::Assistant,
                thinking: if !thinking_so_far.is_empty() {
                    Some(thinking_so_far)
                } else {
                    None
                },
                content: content_so_far,
            });

            self.trace.metadata.modified = now;

            if let Err(err) = super::persistence::save_to_directory(&self.trace).await {
                log::error!(
                    "Failed to save trace {} to file",
                    self.trace.metadata.id.hyphenated()
                );
                let _ = self.event_broadcast.send(TraceEvent::Error {
                    message: err.to_string(),
                });
            }

            complete_notify.notify_waiters();
        }
    }
}
