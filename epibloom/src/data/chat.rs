mod db;
mod session;

use std::{collections::HashMap, fmt, sync::Arc};

use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use time::macros::format_description;
use time::OffsetDateTime;
use tokio::sync::{broadcast, RwLock};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::backends;
use crate::data;
use crate::util;

pub use db::db_init;
pub use session::{Session, ChatEvent, ChatStore};

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
    pub thinking: Option<String>,
    pub text: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub modified: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChatMetadata {
    pub id: Uuid,
    pub name: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub modified: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Chat {
    #[serde(flatten)]
    pub metadata: ChatMetadata,
    pub messages: Vec<Message>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("chat not found: {0}")]
    ChatNotFound(Uuid),

    #[error("chat {0} has no messages")]
    ChatHasNoMessages(Uuid),

    #[error("message not found: chat {chat_id}, message position {position}")]
    MessageNotFound { chat_id: Uuid, position: usize },

    #[error("chat is already generating")]
    ChatAlreadyGenerating,

    #[error("thinking was supplied for a non-assistant message")]
    ThinkingSuppliedForNonAssistantMessage,

    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
}
