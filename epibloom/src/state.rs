//! State objects that are named JSON values that broadcast changes to
//subscribed clients, in the form of JSON Patch blobs.

use std::collections::HashMap;
use std::sync::Arc;

use json_patch::Patch;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, RwLock};
use utoipa::ToSchema;

/// Manages all state objects in the system.
pub struct StateStore {
    objects: RwLock<HashMap<String, Arc<StateObject>>>,
}

impl StateStore {
    pub fn new() -> Self {
        Self {
            objects: RwLock::new(HashMap::new()),
        }
    }

    /// Get or create a state object by name.
    pub async fn get_or_create(&self, name: &str) -> Arc<StateObject> {
        let mut objects = self.objects.write().await;

        if let Some(obj) = objects.get(name) {
            Arc::clone(obj)
        } else {
            let new_obj = Arc::new(StateObject::new(name));
            objects.insert(name.to_string(), Arc::clone(&new_obj));
            new_obj
        }
    }
}

/// A single state object with a JSON value.
pub struct StateObject {
    name: String,

    /// The JSON value.
    value: RwLock<serde_json::Value>,

    /// Broadcast channel for state changes.
    event_broadcast: broadcast::Sender<StateEvent>,
}

impl StateObject {
    fn new(name: &str) -> Self {
        let (broadcast, _) = broadcast::channel(128);

        Self {
            name: name.to_string(),
            value: RwLock::new(serde_json::Value::Null),
            event_broadcast: broadcast,
        }
    }

    /// Read the current value.
    pub async fn read(&self) -> serde_json::Value {
        self.value.read().await.clone()
    }

    /// Write a new value and broadcast it to subscribers.
    pub async fn update<F, E>(&self, update: F) -> Result<(), E>
    where
        F: AsyncFnOnce(serde_json::Value) -> Result<serde_json::Value, E>,
        E: std::error::Error + Send + Sync,
    {
        let mut value_guard = self.value.write().await;

        let old_value = value_guard.clone();
        let new_value = update(old_value.clone()).await?;

        *value_guard = new_value.clone();

        // Compute JSON Patch diff.
        let patch = json_patch::diff(&old_value, &new_value);

        let event = StateEvent::Patch {
            object: self.name.clone(),
            patch,
        };

        // Broadcast the event.
        let _ = self.event_broadcast.send(event);

        Ok(())
    }

    /// Subscribe to state changes. Returns stream of current value + future updates.
    pub async fn subscribe(&self) -> impl futures::Stream<Item = StateEvent> {
        use async_stream::stream;

        let current_value = self.read().await;
        let name = self.name.clone();
        let mut rx = self.event_broadcast.subscribe();

        stream! {
            use tokio::sync::broadcast::error::RecvError;

            // Yield current state first.
            yield StateEvent::Value {
                object: name.clone(),
                value: current_value,
            };

            // Then stream future updates.
            loop {
                match rx.recv().await {
                    Ok(event) => yield event,
                    Err(RecvError::Lagged(n)) => {
                        log::error!("State object '{}' stream lagged {} messages", name, n);
                    }
                    Err(RecvError::Closed) => break,
                }
            }
        }
    }
}

/// Event sent to subscribers when state changes.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StateEvent {
    Value {
        object: String,
        value: serde_json::Value,
    },

    /// State was updated (JSON Patch diff sent to allow for large objects
    /// changing only a little).
    Patch {
        object: String,

        #[schema(value_type = Vec<Object>)]
        patch: Patch,
    },
}
