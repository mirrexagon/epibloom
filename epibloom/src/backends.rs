//! LLM backends.
//!
//! Each backend has a Config struct that implements
//! `schemars::JsonSchema` that can get reported to the UI to dynamically generate
//! the configuration UI for that backend.

pub mod anthropic;
pub mod featherless;

use futures::Stream;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::data::chat::Chat;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub enum GenerateChunk {
    ThinkingDelta(String),
    MessageDelta(String),
}

pub trait Backend {
    type Config: JsonSchema;
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

    fn generate<'a>(
        &'a self,
        config: &'a Self::Config,
        chat: &'a Chat,
    ) -> impl Stream<Item = Result<GenerateChunk, Self::Error>> + Send + 'a;
}
