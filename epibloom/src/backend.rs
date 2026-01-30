//! LLM backends.
//!
//! Each backend has a Config struct that implements
//! `schemars::JsonSchema` that can get reported to the UI to dynamically generate
//! the configuration UI for that backend.

pub mod anthropic;
pub mod featherless;

use std::pin::Pin;
use std::sync::Arc;

use futures::Stream;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::chat::Chat;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub enum GenerateChunk {
    ThinkingDelta(String),
    MessageDelta(String),
}

pub trait Backend {
    type Config: JsonSchema + Clone;
    type Error: std::error::Error;

    // TODO: On config structs, use
    // https://graham.cool/schemars/deriving/attributes/#extend to add a field for
    // the UI control type (texbox, spinner, etc.). There are also validators like
    // https://graham.cool/schemars/deriving/attributes/#range
    // Description seems like it will come from doc comments.
    /*
    fn get_config_schema() -> RootSchema {
        schema_for!(Self::Config)
    }
    */

    fn generate(
        &self,
        config: &Self::Config,
        chat: &Chat,
    ) -> Pin<Box<dyn Stream<Item = Result<GenerateChunk, Self::Error>> + Send>>;
}

pub struct Backends {
    backends: RwLock<Vec<Backend>>,
}

impl Backends {
    pub fn new() -> Self {
        Self {
            backends: RwLock::new(Vec::new()),
        }
    }

    /// Get a backend, or construct it if it hasn't been instantiated yet.
    pub fn get<B: Backend>(&self) -> Result<Arc<dyn Backend>, std::io::Error> {}
}
