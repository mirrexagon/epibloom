use std::collections::HashMap;
use std::sync::Arc;

use async_stream::stream;
use futures::Stream;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, Mutex, RwLock};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::backend::anthropic::{self, messages::request, AnthropicApi};

use super::conversation::{Message, Role, Trace, TraceMetadata};
use super::session::TraceEvent;
use super::session::Session;
use super::Error;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TraceStoreEvent {
    Added { trace: TraceMetadata },
}

pub struct TraceStore {
    sessions: RwLock<HashMap<Uuid, Arc<RwLock<Session>>>>,
    anthropic: Mutex<Option<Arc<AnthropicApi>>>,
    event_broadcast: broadcast::Sender<TraceStoreEvent>,
}

impl TraceStore {
    pub fn new() -> Self {
        let (event_broadcast, _) = broadcast::channel(32);

        Self {
            sessions: RwLock::new(HashMap::new()),
            anthropic: Mutex::new(None),
            event_broadcast,
        }
    }

    pub async fn load_traces_from_directory(&self) -> Result<(), Error> {
        let traces = super::persistence::load_all_from_directory().await?;

        for trace in traces {
            self.add_trace(trace).await?;
        }

        Ok(())
    }

    pub async fn list_traces(&self) -> Result<Vec<TraceMetadata>, Error> {
        let sessions = self.sessions.read().await;

        // TODO: This is inefficient since it has to lock every trace. Is there a
        // better way to store traces so the metadata is more easily accessible?
        let mut traces = Vec::new();
        for (_, session_arc) in sessions.iter() {
            let session = session_arc.read().await;
            traces.push(session.trace().metadata.clone());
        }

        Ok(traces)
    }

    pub async fn get_session(&self, trace_id: Uuid) -> Result<Arc<RwLock<Session>>, Error> {
        let sessions = self.sessions.read().await;

        match sessions.get(&trace_id) {
            Some(session_arc) => Ok(Arc::clone(session_arc)),
            None => Err(Error::TraceNotFound),
        }
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

    pub async fn get_trace_store_event_stream(&self) -> impl Stream<Item = TraceStoreEvent> {
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
                trace: session.trace().metadata.clone(),
            });
        }

        let mut rx = self.event_broadcast.subscribe();

        stream! {
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
        }
    }

    /// Create a trace from the given messages, persist it, and immediately
    /// start Anthropic generation. Returns the new trace's ID and a receiver
    /// for streaming generation events.
    pub async fn generate(
        &self,
        trace_messages: Vec<Message>,
        system: Option<String>,
        anthropic_messages: Vec<request::Message>,
        config: anthropic::Config,
    ) -> Result<(Uuid, broadcast::Receiver<TraceEvent>), Error> {
        let trace = Trace {
            metadata: TraceMetadata::new(None),
            messages: trace_messages,
        };
        let trace_id = trace.metadata.id;

        super::persistence::save_to_directory(&trace).await?;
        self.add_trace(trace).await?;

        // Lazily initialise the Anthropic client.
        let backend = {
            let mut guard = self.anthropic.lock().await;
            if guard.is_none() {
                log::info!("Initialising Anthropic backend");
                let api = AnthropicApi::new_from_env()
                    .map_err(|err| Error::Backend(err.to_string()))?;
                *guard = Some(Arc::new(api));
            }
            Arc::clone(guard.as_ref().unwrap())
        };

        let session_arc = self.get_session(trace_id).await?;

        let (trace_snapshot, rx) = {
            let mut session = session_arc.write().await;
            session.begin_generation()
        };

        let message_position = trace_snapshot.messages.len();
        let stream = backend.generate(&config, system, anthropic_messages);

        tokio::spawn(async move {
            {
                let session = session_arc.read().await;
                session.broadcast_message_start(message_position, Role::Assistant);
            }

            use futures::stream::StreamExt;
            futures::pin_mut!(stream);

            while let Some(result) = stream.next().await {
                let mut session = session_arc.write().await;
                match result {
                    Ok(chunk) => session.process_chunk(&chunk),
                    Err(err) => {
                        log::error!("Error in generation stream: {err}");
                        session.broadcast_error(err.to_string());
                    }
                }
            }

            {
                let session = session_arc.read().await;
                session.broadcast_message_end(message_position);
            }

            {
                let mut session = session_arc.write().await;
                session.finish_generation().await;
            }
        });

        Ok((trace_id, rx))
    }

    // TODO: Cancel a trace that is generating
}
