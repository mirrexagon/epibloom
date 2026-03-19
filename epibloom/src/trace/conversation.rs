use std::fmt;

use serde::{Deserialize, Serialize};
use time::macros::format_description;
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::{NoContext, Timestamp, Uuid};

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize, ToSchema)]
pub enum Role {
    #[serde(rename = "system")]
    System,
    #[serde(rename = "user")]
    User,
    #[serde(rename = "assistant")]
    Assistant,
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match *self {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
        };

        write!(f, "{s}")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Message {
    pub role: Role,
    pub content: String,
    pub thinking: Option<String>,
    // TODO: Add error for any error that occurred generating this message.
    // Maybe make content an Option in case the error happens before any message
    // chunks come in?
    // TODO: Track when generation started and when it ended.
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TraceMetadata {
    pub id: Uuid,
    pub name: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub modified: OffsetDateTime,
}

impl TraceMetadata {
    pub fn new(name: Option<&str>) -> Self {
        let current_datetime = OffsetDateTime::now_local().unwrap();
        let current_unix_timestamp = current_datetime.unix_timestamp();
        let uuid_timestamp = Timestamp::from_unix(NoContext, current_unix_timestamp as u64, 0);

        let id = Uuid::new_v7(uuid_timestamp);
        let name = if let Some(name) = name {
            name.to_owned()
        } else {
            let dtfmt =
                format_description!(version = 2, "[year]-[month]-[day] [hour]:[minute]:[second]");
            current_datetime.format(&dtfmt).unwrap()
        };

        Self {
            id,
            name,
            created: current_datetime,
            modified: current_datetime,
        }
    }
}

// TODO: Rename to Conversation and rename session to Trace
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Trace {
    #[serde(flatten)]
    pub metadata: TraceMetadata,
    pub messages: Vec<Message>,
}

impl Trace {
    pub fn new(name: Option<&str>) -> Self {
        Self {
            metadata: TraceMetadata::new(name),
            messages: Vec::new(),
        }
    }
}
