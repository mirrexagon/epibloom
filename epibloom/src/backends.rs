//! LLM backends.
//!
//! Each backend has a Config struct that implements
//! `schemars::JsonSchema` that can get reported to the UI to dynamically generate
//! the configuration UI for that backend.

pub mod anthropic;
pub mod featherless;

use std::fmt;

use futures::Stream;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize, ToSchema)]
pub enum Role {
    #[serde(rename = "system")]
    System,
    #[serde(rename = "assistant")]
    Assistant,
    #[serde(rename = "user")]
    User,
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match *self {
            Role::System => "system",
            Role::Assistant => "assistant",
            Role::User => "user",
        };

        write!(f, "{s}")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Message {
    pub role: Role,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Chat {
    pub messages: Vec<Message>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub enum GenerateChunk {
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
    ) -> impl Stream<Item = Result<GenerateChunk, Self::Error>> + 'a;
}
