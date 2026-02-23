use std::collections::HashMap;
use std::sync::Arc;

use async_stream::stream;
use futures::Stream;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, Mutex, RwLock};
use typemap_rev::TypeMapKey;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::backend::{Backend, Backends};

use super::conversation::{Role, Trace, TraceMetadata};
use super::session::TraceEvent;
use super::session::{GenerationState, Session};
use super::Error;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TraceStoreEvent {
    Added { trace: TraceMetadata },
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

    pub async fn load_traces_from_directory(&self) -> Result<(), Error> {
        let traces = super::persistence::load_all_from_directory().await?;

        for trace in traces {
            self.add_trace(trace).await?;
        }

        Ok(())
    }

    pub async fn save_trace(&self, trace_id: Uuid) -> Result<(), Error> {
        let sessions = self.sessions.read().await;
        let session_arc = sessions.get(&trace_id).ok_or(Error::TraceNotFound)?;
        let session = session_arc.read().await;
        super::persistence::save_to_directory(session.trace()).await?;
        Ok(())
    }

    pub async fn save_all_traces(&self) -> Result<(), Error> {
        let sessions = self.sessions.read().await;

        for (_, session_arc) in sessions.iter() {
            let session = session_arc.read().await;
            super::persistence::save_to_directory(session.trace()).await?;
        }

        Ok(())
    }

    pub async fn has_trace(&self, trace_id: Uuid) -> bool {
        let sessions = self.sessions.read().await;
        sessions.contains_key(&trace_id)
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

    /// Create a new blank trace and add it as a session.
    pub async fn new_trace(&self, name: Option<&str>) -> Result<Uuid, Error> {
        let trace = Trace::new(name);
        let trace_metadata = trace.metadata.clone();

        log::info!(
            "Creating new trace: {}: {}",
            trace_metadata.id,
            trace_metadata.name
        );

        super::persistence::save_to_directory(&trace).await?;
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

    pub async fn start_generating<B: Backend + TypeMapKey<Value = Arc<B>> + Send + Sync>(
        &self,
        trace_id: Uuid,
        backend_config: B::Config,
    ) -> Result<broadcast::Receiver<TraceEvent>, Error>
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

        let (trace, rx) = {
            let mut session = session_arc.write().await;
            session.begin_generation()?
        };

        let message_position = trace.messages.len();
        let stream = backend.generate(&backend_config, &trace);

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

            // Finish generation: commit message, save, notify waiters.
            {
                let mut session = session_arc.write().await;
                session.finish_generation().await;
            }
        });

        Ok(rx)
    }

    pub async fn wait_for_generate_complete(&self, trace_id: Uuid) -> Result<(), Error> {
        let session = self.get_session(trace_id).await?;

        let notified = {
            let session = session.read().await;

            match session.generation_state() {
                GenerationState::Idle => return Ok(()),
                GenerationState::Generating {
                    complete_notify, ..
                } => Arc::clone(complete_notify).notified_owned(),
            }
        };

        notified.await;

        Ok(())
    }

    // TODO: Cancel a trace that is generating
}
