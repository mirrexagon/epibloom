//! LLM backends.
//!
//! Each backend should have a Config struct that implements
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
