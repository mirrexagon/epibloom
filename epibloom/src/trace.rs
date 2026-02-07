mod session;

use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};
use time::macros::format_description;
use time::OffsetDateTime;
use tokio::fs;
use utoipa::ToSchema;
use uuid::{NoContext, Timestamp, Uuid};

pub use session::{TraceEvent, TraceStore, TraceStoreEvent};

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
    pub text: String,
    pub thinking: Option<String>,
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

    pub async fn load_all_from_traces_directory() -> Result<Vec<Self>, Error> {
        let mut traces = Vec::new();

        let mut dir = fs::read_dir(crate::TRACES_DIR.get().unwrap()).await?;
        while let Some(entry) = dir.next_entry().await? {
            traces.push(Self::load_from_file(entry.path()).await?);
        }

        Ok(traces)
    }

    pub async fn load_from_traces_directory(trace_id: Uuid) -> Result<Self, Error> {
        let mut file_path = crate::TRACES_DIR.get().unwrap().clone();
        file_path.push(format!("{}.json", trace_id.hyphenated()));

        Self::load_from_file(&file_path).await
    }

    pub async fn save_to_traces_directory(&self) -> Result<(), Error> {
        let mut file_path = crate::TRACES_DIR.get().unwrap().clone();
        file_path.push(format!("{}.json", self.metadata.id.hyphenated()));
        self.save_to_file(&file_path).await?;
        Ok(())
    }

    pub async fn load_from_file<P: AsRef<Path>>(file_path: P) -> Result<Self, Error> {
        let text = fs::read(file_path).await?;
        let trace = serde_json::from_slice(&text)?;
        Ok(trace)
    }

    pub async fn save_to_file<P: AsRef<Path>>(&self, file_path: P) -> Result<(), Error> {
        fs::write(&file_path, serde_json::to_string_pretty(self)?).await?;
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    // TODO: Remove context information from errors, always log that at the
    // caller receiving the error. Getting traces failing in the API already
    // does.
    #[error("trace not found: {0}")]
    TraceNotFound(Uuid),

    #[error("trace {0} has no messages")]
    TraceHasNoMessages(Uuid),

    #[error("message not found: trace {trace_id}, message position {position}")]
    MessageNotFound { trace_id: Uuid, position: usize },

    #[error("can't modify trace while generating")]
    CantModifyTraceWhileGenerating,

    #[error("thinking was supplied for a non-assistant message")]
    ThinkingSuppliedForNonAssistantMessage,

    #[error("Backend error: {0}")]
    Backend(String),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
