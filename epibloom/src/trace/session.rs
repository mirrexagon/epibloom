use std::collections::HashMap;
use std::sync::Arc;

use async_stream::stream;
use futures::Stream;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, Mutex, RwLock};
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

    pub async fn new_trace(&self, name: Option<&str>) -> Result<Uuid, Error> {
        let trace = Trace::new(name);
        let new_trace_id = trace.metadata.id;

        log::info!("Creating trace: {new_trace_id}: '{}'", trace.metadata.name);

        trace.save_to_traces_directory().await?;
        self.add_trace(trace).await?;
        Ok(new_trace_id)
    }

    async fn add_trace(&self, trace: Trace) -> Result<(), Error> {
        let (broadcast, _) = broadcast::channel(1024);
        let new_trace_metadata = trace.metadata.clone();
        let new_trace_id = new_trace_metadata.id;
        let new_session = Session {
            trace,
            current_assistant_message: String::new(),
            current_assistant_thinking: String::new(),
            broadcast,
            state: State::Idle,
        };

        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(new_trace_id, Arc::new(RwLock::new(new_session)));

            // We don't care if there are no receivers.
            let _ = self.event_broadcast.send(TraceStoreEvent::Added {
                trace: new_trace_metadata,
            });
        }

        Ok(())
    }

    async fn get_session_arc(&self, trace_id: Uuid) -> Result<Arc<RwLock<Session>>, Error> {
        let sessions = self.sessions.read().await;

        match sessions.get(&trace_id) {
            Some(session_arc) => Ok(session_arc.clone()),
            None => Err(Error::TraceNotFound(trace_id)),
        }
    }

    pub async fn get_trace_copy(&self, trace_id: Uuid) -> Result<Trace, Error> {
        let session_arc = self.get_session_arc(trace_id).await?;
        let session = session_arc.read().await;
        Ok(session.trace.clone())
    }

    // TODO: Maybe just set the entire trace or a single message instead of having functions to push messages and stuff.
    // The Lua API can update last modified times etc.
    // The Lua LuaTrace object can have messages indexed by integers directly in
    // the trace object. Only problem is doing things
    // like `trace[1].text = "text"` directly.
    // Also should probably be more like parent object is a trace store trace
    // reference and the individual trace object is just a trace, since it has
    // metadata like ID and times. Or you get and set these trace copy objects on the
    // trace store directly.

    pub async fn push_message(
        &self,
        trace_id: Uuid,
        role: Role,
        text: &str,
        thinking: Option<&str>,
    ) -> Result<usize, Error> {
        if thinking.is_some() && role != Role::Assistant {
            return Err(Error::ThinkingSuppliedForNonAssistantMessage);
        }

        let session_arc = self.get_session_arc(trace_id).await?;
        let mut session = session_arc.write().await;

        if session.get_state() == State::Generating {
            return Err(Error::TraceAlreadyGenerating);
        }

        let now = time::OffsetDateTime::now_local().unwrap();

        session.trace.messages.push(Message {
            role,
            thinking: thinking.map(|s| s.to_owned()),
            text: text.to_owned(),
            created: now,
            modified: now,
        });
        session.trace.metadata.modified = now;
        session.trace.save_to_traces_directory().await?;

        let new_message_position = session.trace.messages.len() - 1;

        let _ = session.broadcast.send(match role {
            Role::System | Role::User => TraceEvent::UserMessageStart {
                position: new_message_position,
            },
            Role::Assistant => TraceEvent::AssistantMessageStart {
                position: new_message_position,
            },
        });

        if let Some(thinking) = thinking {
            let _ = session.broadcast.send(TraceEvent::AssistantThinkingDelta {
                delta: thinking.to_owned(),
            });
        }

        let _ = session.broadcast.send(match role {
            Role::System | Role::User => TraceEvent::UserMessageDelta {
                delta: text.to_owned(),
            },
            Role::Assistant => TraceEvent::AssistantMessageDelta {
                delta: text.to_owned(),
            },
        });

        let _ = session.broadcast.send(match role {
            Role::System | Role::User => TraceEvent::UserMessageStart {
                position: new_message_position,
            },
            Role::Assistant => TraceEvent::AssistantMessageStart {
                position: new_message_position,
            },
        });

        Ok(new_message_position)
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
            let session = session_arc.read().await;
            initial_trace_store_events.push(TraceStoreEvent::Added {
                trace: session.trace.metadata.clone(),
            });
        }

        let mut rx = self.event_broadcast.subscribe();

        // TODO: This is inefficient since it has to lock every trace. Is there a
        // better way to store traces so the metadata is more easily accessible?

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

    /// Get a stream that yields events corresponding to existing messages, then any future message chunks.
    pub async fn get_trace_stream(
        &self,
        trace_id: Uuid,
    ) -> Result<impl Stream<Item = TraceEvent>, Error> {
        let session_arc = self.get_session_arc(trace_id).await?;

        Ok(stream! {
            let (mut rx, past_messages, current_assistant_thinking, current_assistant_message, state) = {
                let session = session_arc.read().await;
                let (current_assistant_thinking, current_assistant_message) =
                    session.get_in_progress_message();
                (
                    session.get_new_receiver(),
                    session.get_complete_messages(),
                    current_assistant_thinking,
                    current_assistant_message,
                    session.get_state(),
                )
            };

            // Send past messages.
            for (position, message) in past_messages.iter().enumerate() {
                match message.role {
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

                    Role::System => {}
                }
            }

            // Send current message so far.
            if let State::Generating = state {
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

    pub async fn start_generating<B: Backend + TypeMapKey<Value = Arc<B>> + Send + Sync>(
        &self,
        trace_id: Uuid,
        backend_config: B::Config,
    ) -> Result<(), Error>
    where
        <B as Backend>::Config: Send + Sync + 'static,
        <B as Backend>::Error: Send + Sync + 'static,
    {
        let session_arc = self.get_session_arc(trace_id).await?;
        let backend = self
            .backends
            .lock()
            .await
            .get::<B>()
            .map_err(|err| Error::Backend(err.to_string()))?;
        Session::start_generating::<B>(session_arc, backend, backend_config).await?;
        Ok(())
    }

    // TODO: Cancel a trace that is generating
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Idle,
    Generating,
}

struct Session {
    trace: Trace,

    current_assistant_thinking: String,
    /// Current incomplete assistant message. Will be added to the trace once it is complete.
    current_assistant_message: String,

    broadcast: broadcast::Sender<TraceEvent>,
    state: State,
}

impl Session {
    fn get_new_receiver(&self) -> broadcast::Receiver<TraceEvent> {
        self.broadcast.subscribe()
    }

    fn get_complete_messages(&self) -> Vec<Message> {
        self.trace.messages.clone()
    }

    fn get_in_progress_message(&self) -> (String, String) {
        (
            self.current_assistant_thinking.clone(),
            self.current_assistant_message.clone(),
        )
    }

    fn get_state(&self) -> State {
        self.state
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
                return Err(Error::TraceAlreadyGenerating);
            }

            session.state = State::Generating;
            session.trace.clone()
        };

        let stream = backend.generate(&backend_config, &trace);

        tokio::spawn(async move {
            let _ =
                process_generation_stream(session_arc.clone(), stream, trace.messages.len()).await;
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
                    created: now,
                    modified: now,
                });

                session.current_assistant_message.truncate(0);
                session.current_assistant_thinking.truncate(0);

                session.trace.metadata.modified = now;
                session.state = State::Idle;

                if let Err(err) = session.trace.save_to_traces_directory().await {
                    let _ = session.broadcast.send(TraceEvent::Error {
                        message: err.to_string(),
                    });
                }
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
        let _ = session.broadcast.send(TraceEvent::AssistantMessageStart {
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
                    let _ = session.broadcast.send(TraceEvent::AssistantThinkingDelta {
                        delta: delta.clone(),
                    });
                }
                GenerateChunk::MessageDelta(ref delta) => {
                    session.current_assistant_message.push_str(delta);
                    let _ = session.broadcast.send(TraceEvent::AssistantMessageDelta {
                        delta: delta.clone(),
                    });
                }
            },
            Err(err) => {
                let _ = session.broadcast.send(TraceEvent::Error {
                    message: err.to_string(),
                });
                return Err(Box::new(err));
            }
        }
    }

    {
        let session = session_arc.write().await;
        let _ = session.broadcast.send(TraceEvent::AssistantMessageEnd {
            position: message_position,
        });
    }

    Ok(())
}
