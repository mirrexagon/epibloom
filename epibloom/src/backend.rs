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
use typemap_rev::{TypeMap, TypeMapKey};
use utoipa::ToSchema;

use crate::trace::Trace;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub enum GenerateChunk {
    ThinkingDelta(String),
    MessageDelta(String),
}

pub trait Backend: TypeMapKey + Send + Sync {
    type Config: JsonSchema + Clone;
    type Error: std::error::Error + Send + Sync;

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

    fn new_with_key_from_env() -> Result<Self, Self::Error>
    where
        Self: Sized;

    fn generate(
        &self,
        config: &Self::Config,
        trace: &Trace,
    ) -> Pin<Box<dyn Stream<Item = Result<GenerateChunk, Self::Error>> + Send>>;
}

impl TypeMapKey for anthropic::AnthropicApi {
    type Value = Arc<anthropic::AnthropicApi>;
}

impl TypeMapKey for featherless::FeatherlessApi {
    type Value = Arc<featherless::FeatherlessApi>;
}

pub struct Backends {
    backends: typemap_rev::TypeMap,
}

impl Backends {
    pub fn new() -> Self {
        Self {
            backends: TypeMap::new(),
        }
    }

    /// Get a backend, or construct it if it hasn't been instantiated yet.
    pub fn get<B: Backend + TypeMapKey<Value = Arc<B>>>(
        &mut self,
    ) -> Result<Arc<B>, Box<dyn std::error::Error + Send + Sync>> {
        match self.backends.get::<B>() {
            Some(backend_arc) => Ok(Arc::clone(backend_arc)),
            None => {
                log::info!(
                    "Creating new backend on-demand: {}",
                    std::any::type_name::<B>()
                );

                let new_backend = Arc::new(B::new_with_key_from_env()?);
                self.backends.insert::<B>(Arc::clone(&new_backend));
                Ok(new_backend)
            }
        }
    }
}
