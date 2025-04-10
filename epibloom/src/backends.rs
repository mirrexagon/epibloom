//! LLM backends.
//!
//! Each backend has a Config struct that implements
//! `schemars::JsonSchema` that can get reported to the UI to dynamically generate
//! the configuration UI for that backend.

pub mod anthropic;

use schemars::{schema::RootSchema, schema_for, JsonSchema};

use crate::data;

pub trait Backend {
    type Config: JsonSchema;
    type Error: std::error::Error;

    fn get_config_schema() -> RootSchema {
        schema_for!(Self::Config)
    }

    async fn generate(
        &self,
        config: &Self::Config,
        chat: &data::chat::Chat,
    ) -> Result<String, Self::Error>;
}

// TODO: On config structs, use
// https://graham.cool/schemars/deriving/attributes/#extend to add a field for
// the UI control type (texbox, spinner, etc.). There are also validators like
// https://graham.cool/schemars/deriving/attributes/#range
// Description seems like it will come from doc comments.
