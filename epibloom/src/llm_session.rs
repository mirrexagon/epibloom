use std::fmt;

use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use time::macros::format_description;
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::backends;
use crate::util;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LlmSessionMetadata {
    pub id: Uuid,
    pub name: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub modified: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LlmSession {
    #[serde(flatten)]
    pub metadata: LlmSessionMetadata,
    pub chat: backends::Chat,
}
