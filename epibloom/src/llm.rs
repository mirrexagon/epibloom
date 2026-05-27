//! LLM generation that writes results to state objects.
//!
//! Each call to [`LlmStore::generate`] creates two state objects:
//! - `llm-trace:<uuid>` — the full trace (input messages + streaming response).
//! - `llm-trace-index` — append-only list of trace metadata for discovery.

use std::sync::Arc;

use futures::StreamExt;
use time::macros::format_description;
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;

use crate::{
    backend::anthropic::{
        self,
        messages::{request, GenerateChunk},
        AnthropicApi,
    },
    state::StateStore,
};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// An input message to record in the trace state object.
pub struct TraceMessage {
    pub role: String,
    pub content: String,
    pub thinking: Option<String>,
}

/// A single chunk streamed back to the Lua handle during generation.
pub enum LlmChunk {
    Thinking { delta: String },
    Message { delta: String },
    Error { message: String },
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Backend error: {0}")]
    Backend(String),
}

// ---------------------------------------------------------------------------
// LlmStore
// ---------------------------------------------------------------------------

pub struct LlmStore {
    anthropic: Mutex<Option<Arc<AnthropicApi>>>,
    state_store: Arc<StateStore>,
}

impl LlmStore {
    pub fn new(state_store: Arc<StateStore>) -> Self {
        Self {
            anthropic: Mutex::new(None),
            state_store,
        }
    }

    /// Start an LLM generation.
    ///
    /// Creates the `llm-trace:<uuid>` and `llm-trace-index` state objects,
    /// spawns a task that streams chunks into the trace object, and returns
    /// the trace UUID string plus an [`mpsc::Receiver`] that delivers the same
    /// chunks to the caller (for the Lua handle).
    pub async fn generate(
        &self,
        trace_messages: Vec<TraceMessage>,
        system: Option<String>,
        anthropic_messages: Vec<request::Message>,
        config: anthropic::Config,
    ) -> Result<(String, mpsc::Receiver<LlmChunk>), Error> {
        // ---- Unique ID & human-readable name --------------------------------
        let uuid = Uuid::now_v7();
        let uuid_str = format!("{}", uuid.hyphenated());

        let now = time::OffsetDateTime::now_local().unwrap();
        let dtfmt =
            format_description!(version = 2, "[year]-[month]-[day] [hour]:[minute]:[second]");
        let name = now.format(dtfmt).unwrap();
        let created = now
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap();

        // ---- Build initial state value --------------------------------------
        // The assistant entry is always appended last; we note its index so
        // the spawned task can patch it by position.
        let assistant_idx = trace_messages.len();

        let mut messages_json: Vec<serde_json::Value> = trace_messages
            .iter()
            .map(|msg| {
                let mut obj = serde_json::json!({
                    "role": msg.role,
                    "content": [&msg.content],
                });
                if let Some(thinking) = &msg.thinking {
                    obj["thinking"] = serde_json::json!([thinking]);
                }
                obj
            })
            .collect();

        messages_json.push(serde_json::json!({
            "role": "assistant",
            "content": [],
            "thinking": [],
            "generating": true,
        }));

        let initial_value = serde_json::json!({
            "id": uuid_str,
            "name": name,
            "created": created,
            "messages": messages_json,
        });

        // ---- Write trace state object ---------------------------------------
        let trace_obj_name = format!("llm-trace:{}", uuid_str);
        let trace_obj = self.state_store.get_or_create(&trace_obj_name).await;
        trace_obj
            .update(async move |_: serde_json::Value| {
                Ok::<_, std::convert::Infallible>(initial_value)
            })
            .await
            .unwrap(); // Infallible

        // ---- Append to discovery index -------------------------------------
        let index_obj = self.state_store.get_or_create("llm-trace-index").await;
        let entry = serde_json::json!({
            "id": uuid_str,
            "name": name,
            "created": created,
        });
        index_obj
            .update(async move |old: serde_json::Value| {
                let mut arr = match old {
                    serde_json::Value::Array(a) => a,
                    _ => Vec::new(), // handles null / first-ever entry
                };
                arr.push(entry);
                Ok::<_, std::convert::Infallible>(serde_json::Value::Array(arr))
            })
            .await
            .unwrap(); // Infallible

        // ---- Lazily initialise the Anthropic client ------------------------
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

        // ---- Spawn generation task -----------------------------------------
        let (tx, rx) = mpsc::channel::<LlmChunk>(1024);
        let stream = backend.generate(&config, system, anthropic_messages);

        tokio::spawn(async move {
            futures::pin_mut!(stream);

            while let Some(result) = stream.next().await {
                match result {
                    Ok(GenerateChunk::ThinkingDelta(delta)) => {
                        let delta_c = delta.clone();
                        trace_obj
                            .update(async move |mut v: serde_json::Value| {
                                append_chunk(&mut v, assistant_idx, "thinking", &delta_c);
                                Ok::<_, std::convert::Infallible>(v)
                            })
                            .await
                            .unwrap();

                        if tx.send(LlmChunk::Thinking { delta }).await.is_err() {
                            break;
                        }
                    }

                    Ok(GenerateChunk::MessageDelta(delta)) => {
                        let delta_c = delta.clone();
                        trace_obj
                            .update(async move |mut v: serde_json::Value| {
                                append_chunk(&mut v, assistant_idx, "content", &delta_c);
                                Ok::<_, std::convert::Infallible>(v)
                            })
                            .await
                            .unwrap();

                        if tx.send(LlmChunk::Message { delta }).await.is_err() {
                            break;
                        }
                    }

                    Err(err) => {
                        let message = err.to_string();
                        log::error!("Error in LLM generation stream: {message}");
                        let msg_c = message.clone();
                        trace_obj
                            .update(async move |mut v: serde_json::Value| {
                                if let Some(msgs) = v.get_mut("messages") {
                                    if let Some(msg) = msgs.get_mut(assistant_idx) {
                                        msg["error"] = serde_json::Value::String(msg_c);
                                        msg["generating"] = serde_json::Value::Bool(false);
                                    }
                                }
                                Ok::<_, std::convert::Infallible>(v)
                            })
                            .await
                            .unwrap();

                        let _ = tx.send(LlmChunk::Error { message }).await;
                        return; // tx dropped → channel closed → Lua handle sees None
                    }
                }
            }

            // Stream ended normally; mark the assistant message as done.
            trace_obj
                .update(async move |mut v: serde_json::Value| {
                    if let Some(msgs) = v.get_mut("messages") {
                        if let Some(msg) = msgs.get_mut(assistant_idx) {
                            msg["generating"] = serde_json::Value::Bool(false);
                        }
                    }
                    Ok::<_, std::convert::Infallible>(v)
                })
                .await
                .unwrap();
            // tx dropped here → channel closed → Lua handle sees None (done)
        });

        Ok((uuid_str, rx))
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Append a string chunk to the named array field of `messages[idx]`.
fn append_chunk(
    v: &mut serde_json::Value,
    msg_idx: usize,
    field: &str,
    chunk: &str,
) {
    if let Some(msgs) = v.get_mut("messages") {
        if let Some(msg) = msgs.get_mut(msg_idx) {
            if let Some(arr) = msg.get_mut(field).and_then(|f| f.as_array_mut()) {
                arr.push(serde_json::Value::String(chunk.to_owned()));
            }
        }
    }
}
