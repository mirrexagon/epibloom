use std::collections::HashMap;
use std::sync::Arc;

use async_stream::stream;
use futures::Stream;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, Mutex, Notify, RwLock};
use typemap_rev::TypeMapKey;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::backend::{Backend, Backends, GenerateChunk};

use super::{Error, Message, Role, Trace, TraceMetadata};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TraceStoreEvent {
    Added { trace: TraceMetadata },
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TraceEvent {
    SystemMessageStart { position: usize },
    SystemMessageDelta { delta: String },
    SystemMessageEnd { position: usize },
    UserMessageStart { position: usize },
    UserMessageDelta { delta: String },
    UserMessageEnd { position: usize },
    AssistantMessageStart { position: usize },
    AssistantThinkingDelta { delta: String },
    AssistantMessageDelta { delta: String },
    AssistantMessageEnd { position: usize },
    Error { message: String },
}

pub struct TraceStore {
    sessions: RwLock<HashMap<Uuid, Arc<RwLock<Session>>>>,
    backends: Mutex<Backends>,
    event_broadcast: broadcast::Sender<TraceStoreEvent>,
}

impl TraceStore {
    pub fn new() -> Self {
        let (event_broadcast, _) = broadcast::channel(32);

        Self {
            sessions: RwLock::new(HashMap::new()),
            backends: Mutex::new(Backends::new()),
            event_broadcast,
        }
    }

    pub async fn load_traces_from_traces_directory(&self) -> Result<(), Error> {
        let traces = Trace::load_all_from_traces_directory().await?;

        for trace in traces {
            self.add_trace(trace).await?;
        }

        Ok(())
    }

    pub async fn save_trace(&self, trace_id: Uuid) -> Result<(), Error> {
        let sessions = self.sessions.read().await;
        let session_arc = sessions
            .get(&trace_id)
            .ok_or(Error::TraceNotFound(trace_id))?;
        let session = session_arc.read().await;
        session.trace.save_to_traces_directory().await?;
        Ok(())
    }

    pub async fn save_all_traces(&self) -> Result<(), Error> {
        let sessions = self.sessions.read().await;

        for (_, session_arc) in sessions.iter() {
            let session = session_arc.read().await;
            session.trace.save_to_traces_directory().await?;
        }

        Ok(())
    }

    pub async fn has_trace(&self, trace_id: Uuid) -> bool {
        let sessions = self.sessions.read().await;
        sessions.contains_key(&trace_id)
    }

    pub async fn list_traces(&self) -> Result<Vec<TraceMetadata>, super::Error> {
        let sessions = self.sessions.read().await;

        // TODO: This is inefficient since it has to lock every trace. Is there a
        // better way to store traces so the metadata is more easily accessible?
        let mut traces = Vec::new();
        for (_, session_arc) in sessions.iter() {
            let session = session_arc.read().await;
            traces.push(session.trace.metadata.clone());
        }

        Ok(traces)
    }

    pub async fn get_session(&self, trace_id: Uuid) -> Result<Arc<RwLock<Session>>, Error> {
        let sessions = self.sessions.read().await;

        match sessions.get(&trace_id) {
            Some(session_arc) => Ok(Arc::clone(session_arc)),
            None => Err(Error::TraceNotFound(trace_id)),
        }
    }

    /// Create a new blank trace and add it as a session.
    pub async fn new_trace(&self, name: Option<&str>) -> Result<Uuid, Error> {
        let trace = Trace::new(name);
        let trace_metadata = trace.metadata.clone();

        log::info!(
            "Creating new trace: {}: {}",
            trace_metadata.id,
            trace_metadata.name
        );

        trace.save_to_traces_directory().await?;
        self.add_trace(trace).await?;

        Ok(trace_metadata.id)
    }

    /// Add a trace as a session.
    async fn add_trace(&self, trace: Trace) -> Result<(), Error> {
        let trace_metadata = trace.metadata.clone();

        let new_session = Session::new(trace);
        log::info!(
            "Adding trace to store: {}: {}",
            trace_metadata.id,
            trace_metadata.name
        );

        let mut sessions = self.sessions.write().await;
        sessions.insert(trace_metadata.id, Arc::new(RwLock::new(new_session)));

        // We don't care if there are no receivers.
        let _ = self.event_broadcast.send(TraceStoreEvent::Added {
            trace: trace_metadata,
        });

        Ok(())
    }

    pub async fn get_trace_store_event_stream(
        &self,
    ) -> Result<impl Stream<Item = TraceStoreEvent>, Error> {
        // Get the current existing sessions. We also want to lock sessions so
        // that no one can modify it while we subscribe to adds/removes and get
        // the current trace list.
        let sessions = self.sessions.read().await;

        let mut initial_trace_store_events = Vec::new();
        for (_trace_id, session_arc) in sessions.iter() {
            // TODO: This is inefficient since it has to lock every trace. Is there a
            // better way to store traces so the metadata is more easily
            // accessible but still kept in sync with messages?

            let session = session_arc.read().await;
            initial_trace_store_events.push(TraceStoreEvent::Added {
                trace: session.trace.metadata.clone(),
            });
        }

        let mut rx = self.event_broadcast.subscribe();

        Ok(stream! {
            // Send current trace list.
            for event in initial_trace_store_events {
                yield event;
            }


            // Send any changes to the trace list.
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

        })
    }

    pub async fn start_generating<B: Backend + TypeMapKey<Value = Arc<B>> + Send + Sync>(
        &self,
        trace_id: Uuid,
        backend_config: B::Config,
    ) -> Result<(), Error>
    where
        <B as Backend>::Config: Send + Sync + 'static,
        <B as Backend>::Error: Send + Sync + 'static,
    {
        let session_arc = self.get_session(trace_id).await?;
        let backend = self
            .backends
            .lock()
            .await
            .get::<B>()
            .map_err(|err| Error::Backend(err.to_string()))?;
        Session::start_generating::<B>(session_arc, backend, backend_config).await?;
        Ok(())
    }

    pub async fn wait_for_generate_complete(&self, trace_id: Uuid) -> Result<(), Error> {
        let session = self.get_session(trace_id).await?;

        let notified = {
            let session = session.read().await;

            if session.state == State::Idle {
                return Ok(());
            }

            let notify = Arc::clone(&session.generate_complete_notify);
            notify.notified_owned()
        };

        notified.await;

        Ok(())
    }

    // TODO: Cancel a trace that is generating
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Idle,
    Generating,
}

pub struct Session {
    trace: Trace,
    state: State,

    /// Current incomplete assistant thinking. Will be added to the trace once it is complete.
    current_assistant_thinking: String,
    /// Current incomplete assistant message. Will be added to the trace once it is complete.
    current_assistant_message: String,

    event_broadcast: broadcast::Sender<TraceEvent>,
    generate_complete_notify: Arc<Notify>,
}

impl Session {
    pub fn new(trace: Trace) -> Self {
        let (broadcast, _) = broadcast::channel(1024);

        Session {
            trace,
            state: State::Idle,
            current_assistant_message: String::new(),
            current_assistant_thinking: String::new(),
            event_broadcast: broadcast,
            generate_complete_notify: Arc::new(Notify::new()),
        }
    }

    pub fn get_message(&self, message_position: usize) -> Option<Message> {
        self.trace.messages.get(message_position).cloned()
    }

    /// Provide no message position to append a message.
    pub async fn set_message(
        &mut self,
        message_position: Option<usize>,
        message: Message,
    ) -> Result<(), Error> {
        // Set message in trace.
        if message.thinking.is_some() && message.role != Role::Assistant {
            return Err(Error::ThinkingSuppliedForNonAssistantMessage);
        }

        if self.state == State::Generating {
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

            _ => {
                return Err(Error::MessageNotFound {
                    trace_id: self.trace.metadata.id,
                    position: message_position,
                })
            }
        }

        // Broadcast that the message was added or changed.
        let message = &self.trace.messages[message_position];

        self.trace.metadata.modified = now;
        self.trace.save_to_traces_directory().await?;

        let _ = self.event_broadcast.send(match message.role {
            Role::System | Role::User => TraceEvent::UserMessageStart {
                position: message_position,
            },
            Role::Assistant => TraceEvent::AssistantMessageStart {
                position: message_position,
            },
        });

        if let Some(ref thinking) = message.thinking {
            let _ = self
                .event_broadcast
                .send(TraceEvent::AssistantThinkingDelta {
                    delta: thinking.to_owned(),
                });
        }

        let _ = self.event_broadcast.send(match message.role {
            Role::System | Role::User => TraceEvent::UserMessageDelta {
                delta: message.text.to_owned(),
            },
            Role::Assistant => TraceEvent::AssistantMessageDelta {
                delta: message.text.to_owned(),
            },
        });

        let _ = self.event_broadcast.send(match message.role {
            Role::System | Role::User => TraceEvent::UserMessageStart {
                position: message_position,
            },
            Role::Assistant => TraceEvent::AssistantMessageStart {
                position: message_position,
            },
        });

        Ok(())
    }

    /// Get a stream that yields events corresponding to existing messages, then any future message chunks.
    pub async fn get_trace_stream(&self) -> Result<impl Stream<Item = TraceEvent>, Error> {
        let mut rx = self.event_broadcast.subscribe();
        let past_messages = self.trace.messages.clone();
        let current_assistant_message = self.current_assistant_message.clone();
        let current_assistant_thinking = self.current_assistant_thinking.clone();
        let current_state = self.state;

        Ok(stream! {
            // Send past messages.
            for (position, message) in past_messages.iter().enumerate() {
                match message.role {
                    Role::System => {
                        yield TraceEvent::SystemMessageStart { position };
                        yield TraceEvent::SystemMessageDelta {
                            delta: message.text.clone(),
                        };
                        yield TraceEvent::SystemMessageEnd { position };
                    }

                    Role::User => {
                        yield TraceEvent::UserMessageStart { position };
                        yield TraceEvent::UserMessageDelta {
                            delta: message.text.clone(),
                        };
                        yield TraceEvent::UserMessageEnd { position };
                    }

                    Role::Assistant => {
                        yield TraceEvent::AssistantMessageStart { position };
                        if let Some(ref thinking) = message.thinking {
                            yield TraceEvent::AssistantThinkingDelta {
                                delta: thinking.clone(),
                            };
                        }
                        yield TraceEvent::AssistantMessageDelta {
                            delta: message.text.clone(),
                        };
                        yield TraceEvent::AssistantMessageEnd { position };
                    }
                }
            }

            // Send current message so far.
            if let State::Generating = current_state {
                yield TraceEvent::AssistantMessageStart {
                    position: past_messages.len(),
                };

                if !current_assistant_thinking.is_empty() {
                    yield TraceEvent::AssistantThinkingDelta {
                        delta: current_assistant_thinking,
                    };
                }

                if !current_assistant_message.is_empty() {
                    yield TraceEvent::AssistantMessageDelta {
                        delta: current_assistant_message,
                    };
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
        })
    }

    async fn start_generating<B: Backend>(
        session_arc: Arc<RwLock<Session>>,
        backend: Arc<B>,
        backend_config: B::Config,
    ) -> Result<(), Error>
    where
        <B as Backend>::Config: Send + Sync + 'static,
        <B as Backend>::Error: Send + Sync + 'static,
    {
        let trace = {
            let mut session = session_arc.write().await;
            if let State::Generating = session.state {
                return Err(Error::CantModifyTraceWhileGenerating);
            }

            session.state = State::Generating;
            session.trace.clone()
        };

        let stream = backend.generate(&backend_config, &trace);

        tokio::spawn(async move {
            let _ =
                process_generation_stream(Arc::clone(&session_arc), stream, trace.messages.len())
                    .await;

            // If an error occurs in process_generation_stream(), a
            // TraceEvent::Error will be sent on the session's broadcast channel.

            // Stream has completed.
            {
                let mut session = session_arc.write().await;

                let new_thinking = session.current_assistant_thinking.clone();
                let new_message = session.current_assistant_message.clone();

                let now = time::OffsetDateTime::now_local().unwrap();

                session.trace.messages.push(Message {
                    role: Role::Assistant,
                    thinking: if !new_thinking.is_empty() {
                        Some(new_thinking)
                    } else {
                        None
                    },
                    text: new_message,
                });

                session.current_assistant_message.truncate(0);
                session.current_assistant_thinking.truncate(0);

                session.trace.metadata.modified = now;
                session.state = State::Idle;

                if let Err(err) = session.trace.save_to_traces_directory().await {
                    log::error!(
                        "Failed to save trace {} to file",
                        session.trace.metadata.id.hyphenated()
                    );
                    let _ = session.event_broadcast.send(TraceEvent::Error {
                        message: err.to_string(),
                    });
                }

                session.generate_complete_notify.notify_waiters();
            }
        });

        Ok(())
    }
}

async fn process_generation_stream<S, E>(
    session_arc: Arc<RwLock<Session>>,
    stream: S,
    message_position: usize,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    S: futures::Stream<Item = Result<GenerateChunk, E>>,
    E: std::error::Error + Send + Sync + 'static,
{
    use futures::stream::StreamExt;

    futures::pin_mut!(stream);

    {
        let session = session_arc.write().await;
        let _ = session
            .event_broadcast
            .send(TraceEvent::AssistantMessageStart {
                position: message_position,
            });
    }

    while let Some(result) = stream.next().await {
        let mut session = session_arc.write().await;
        match result {
            Ok(chunk) => match chunk {
                GenerateChunk::ThinkingDelta(ref delta) => {
                    session.current_assistant_thinking.push_str(delta);

                    // Ignore failure to send as there may be no receivers.
                    let _ = session
                        .event_broadcast
                        .send(TraceEvent::AssistantThinkingDelta {
                            delta: delta.clone(),
                        });
                }
                GenerateChunk::MessageDelta(ref delta) => {
                    session.current_assistant_message.push_str(delta);
                    let _ = session
                        .event_broadcast
                        .send(TraceEvent::AssistantMessageDelta {
                            delta: delta.clone(),
                        });
                }
            },
            Err(err) => {
                log::error!("Error in generation stream: {err}");
                let _ = session.event_broadcast.send(TraceEvent::Error {
                    message: err.to_string(),
                });
                return Err(Box::new(err));
            }
        }
    }

    {
        let session = session_arc.write().await;
        let _ = session
            .event_broadcast
            .send(TraceEvent::AssistantMessageEnd {
                position: message_position,
            });
    }

    Ok(())
}
