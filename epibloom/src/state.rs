//! State objects that are named JSON values that broadcast changes to
//! subscribed clients, in the form of JSON Patch blobs.

use std::path::PathBuf;
use std::sync::Arc;
use std::{collections::HashMap, ffi::OsStr};

use json_patch::Patch;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, RwLock};
use utoipa::ToSchema;

use crate::util::debounce::Debounce;

const PERSIST_PREFIX: &str = "persist:";
const INDEX_OBJECT_NAME: &str = "";

/// Manages all state objects in the system.
pub struct StateStore {
    objects: RwLock<HashMap<String, Arc<StateObject>>>,

    /// A state object with name empty string, which is an array of all
    /// created state object names, always in alphabetical order.
    index: Arc<StateObject>,
}

impl StateStore {
    pub fn new() -> Self {
        let mut objects = HashMap::new();

        let index = Arc::new(StateObject::new_plain(
            INDEX_OBJECT_NAME,
            serde_json::Value::Array(vec![serde_json::Value::String(
                INDEX_OBJECT_NAME.to_owned(),
            )]),
            false,
            PathBuf::new(),
        ));

        objects.insert(INDEX_OBJECT_NAME.to_owned(), Arc::clone(&index));

        Self {
            objects: RwLock::new(objects),
            index,
        }
    }

    pub async fn populate_index(&self) -> Result<(), Error> {
        // Get a list of all persisted state objects we have saved.
        // We let their actual contents be loaded lazily when requested.
        let mut state_objects = Vec::new();
        let mut dir = tokio::fs::read_dir(crate::STATE_PERSIST_DIR.get().unwrap()).await?;
        while let Some(entry) = dir.next_entry().await? {
            let path = entry.path();
            let file_type = entry.file_type().await?;

            if let Some(extension) = path.extension() {
                if file_type.is_file() && extension == "json" {
                    let mut state_object_name = path
                        .file_stem()
                        .unwrap()
                        .to_str()
                        .expect("state object filename is not valid UTF-8")
                        .to_owned();

                    state_object_name.insert_str(0, PERSIST_PREFIX);
                    state_objects.push(state_object_name);
                }
            }
        }

        self.index
            .update_internal(async |_| serde_json::to_value(state_objects))
            .await?;

        Ok(())
    }

    /// Get or create a state object by name.
    pub async fn get_or_create(&self, name: &str) -> Result<Arc<StateObject>, Error> {
        let mut objects = self.objects.write().await;

        if let Some(obj) = objects.get(name) {
            Ok(Arc::clone(obj))
        } else {
            let new_obj = Arc::new(StateObject::new(name).await?);

            objects.insert(name.to_string(), Arc::clone(&new_obj));

            self.index
                .update_internal(async |mut value| -> Result<_, serde_json::Error> {
                    if let serde_json::Value::Array(ref mut index_list) = value {
                        if !index_list.iter().any(|v| {
                            if let serde_json::Value::String(s) = v {
                                s == name
                            } else {
                                false
                            }
                        }) {
                            index_list.push(serde_json::Value::String(name.to_owned()));
                        }
                    } else {
                        panic!("index state objects was not an array");
                    }

                    Ok(value)
                })
                .await?;

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

        Ok(StateObject::new_plain(
            name,
            value,
            persist,
            persist_file_path,
        ))
    }

    fn new_plain(
        name: &str,
        value: serde_json::Value,
        persist: bool,
        persist_file_path: PathBuf,
    ) -> Self {
        let (broadcast, _) = broadcast::channel(128);

        Self {
            name: name.to_string(),
            persist,
            persist_debounce: Debounce::new(tokio::time::Duration::from_millis(500)),
            persist_file_path,
            value: RwLock::new(value),
            event_broadcast: broadcast,
        }
    }

    /// Get the current value.
    pub async fn read(&self) -> serde_json::Value {
        self.value.read().await.clone()
    }

    pub async fn update<F, E>(&self, update: F) -> Result<(), E>
    where
        F: AsyncFnOnce(serde_json::Value) -> Result<serde_json::Value, E>,
        E: std::error::Error + Send + Sync,
    {
        if self.name == INDEX_OBJECT_NAME {
            log::warn!("Tried to update the index state object but that is not allowed");
            return Ok(());
        };

        self.update_internal(update).await
    }

    /// Write a new value and broadcast it to subscribers.
    async fn update_internal<F, E>(&self, update: F) -> Result<(), E>
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

        if !patch.is_empty() {
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
