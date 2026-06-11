//! State objects that are named JSON values that broadcast changes to
//! subscribed clients, in the form of JSON Patch blobs.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use json_patch::Patch;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, RwLock};
use utoipa::ToSchema;

use crate::util::debounce::Debounce;

const PERSIST_PREFIX: &str = "persist:";

/// Manages all state objects in the system.
pub struct StateStore {
    objects: RwLock<HashMap<String, Arc<StateObject>>>,
}

// TODO: Have a state object with name empty string that lists all state objects we know about.

impl StateStore {
    pub fn new() -> Self {
        Self {
            objects: RwLock::new(HashMap::new()),
        }
    }

    /// Get or create a state object by name.
    pub async fn get_or_create(&self, name: &str) -> Result<Arc<StateObject>, Error> {
        let mut objects = self.objects.write().await;

        if let Some(obj) = objects.get(name) {
            Ok(Arc::clone(obj))
        } else {
            let new_obj = Arc::new(StateObject::new(name).await?);
            objects.insert(name.to_string(), Arc::clone(&new_obj));
            Ok(new_obj)
        }
    }
}

/// A single state object with a JSON value.
pub struct StateObject {
    name: String,

    /// Whether to persist this object automatically.
    persist: bool,

    persist_debounce: Debounce,
    persist_file_path: PathBuf,

    /// The JSON value.
    value: RwLock<serde_json::Value>,

    /// Broadcast channel for state changes.
    event_broadcast: broadcast::Sender<StateEvent>,
}

impl StateObject {
    async fn new(name: &str) -> Result<Self, Error> {
        let (broadcast, _) = broadcast::channel(128);

        let persist = name.starts_with(PERSIST_PREFIX);
        let mut persist_file_path = PathBuf::new();
        let mut value = serde_json::Value::Null;

        if persist {
            let name = name.strip_prefix(PERSIST_PREFIX).unwrap().to_owned();
            persist_file_path = crate::STATE_PERSIST_DIR.get().unwrap().clone();
            persist_file_path.push(name);
            persist_file_path.set_extension("json");

            // Try to load existing value from the filesystem, defaulting to
            // null if the file doesn't exist.
            value = match tokio::fs::read_to_string(&persist_file_path).await {
                Ok(contents) => serde_json::from_str(&contents)?,
                Err(err) if err.kind() == tokio::io::ErrorKind::NotFound => serde_json::Value::Null,
                Err(err) => return Err(err.into()),
            };
        }

        Ok(Self {
            name: name.to_string(),
            persist,
            persist_debounce: Debounce::new(tokio::time::Duration::from_secs(1)),
            persist_file_path,
            value: RwLock::new(value),
            event_broadcast: broadcast,
        })
    }

    /// Get the current value.
    pub async fn get(&self) -> serde_json::Value {
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
        // TODO: If the values are the same, don't do any of the rest of this.
        let patch = json_patch::diff(&old_value, &new_value);

        let event = StateEvent::Patch {
            object: self.name.clone(),
            patch,
        };

        // Broadcast the event.
        let _ = self.event_broadcast.send(event);

        if self.persist {
            let file_path = self.persist_file_path.clone();

            // Queue debounced save to disk.
            self.persist_debounce
                .debounce(async move || {
                    let contents = match serde_json::to_string_pretty(&new_value) {
                        Ok(value) => value,
                        Err(err) => {
                            log::error!(
                                "StateObject persistence: failed to serialize state object: {}",
                                err
                            );
                            return;
                        }
                    };

                    if let Err(err) = tokio::fs::write(&file_path, contents).await {
                        log::error!(
                            "StateObject persistence: failed to save file '{}': {}",
                            file_path.display(),
                            err
                        );
                    }
                })
                .await;
        }

        Ok(())
    }

    /// Subscribe to state changes. Returns stream of current value + future updates.
    pub async fn subscribe(&self) -> impl futures::Stream<Item = StateEvent> {
        use async_stream::stream;

        let name = self.name.clone();
        let (current_value, mut rx) = {
            let value_guard = self.value.read().await;
            let current_value = value_guard.clone();
            let rx = self.event_broadcast.subscribe();
            (current_value, rx)
        };

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

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] tokio::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
