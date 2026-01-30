mod session;

use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use time::macros::format_description;
use time::OffsetDateTime;
use tokio::fs;
use utoipa::ToSchema;
use uuid::{NoContext, Timestamp, Uuid};

pub use session::ChatStore;

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
    pub thinking: Option<String>,
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

impl ChatMetadata {
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
pub struct Chat {
    #[serde(flatten)]
    pub metadata: ChatMetadata,
    pub messages: Vec<Message>,
}

impl Chat {
    pub fn new(name: Option<&str>) -> Self {
        Self {
            metadata: ChatMetadata::new(name),
            messages: Vec::new(),
        }
    }

    pub async fn load_all_from_chats_directory() -> Result<Vec<Self>, Error> {
        let mut chats = Vec::new();

        let mut dir = fs::read_dir(crate::CHATS_DIR.get().unwrap()).await?;
        while let Some(entry) = dir.next_entry().await? {
            chats.push(Self::load_from_file(entry.path()).await?);
        }

        Ok(chats)
    }

    pub async fn load_from_chats_directory(chat_id: Uuid) -> Result<Self, Error> {
        let mut file_path = crate::CHATS_DIR.get().unwrap().clone();
        file_path.push(format!("{}.json", chat_id.hyphenated()));

        Self::load_from_file(&file_path).await
    }

    pub async fn save_to_chats_directory(&self) -> Result<(), Error> {
        let mut file_path = crate::CHATS_DIR.get().unwrap().clone();
        file_path.push(format!("{}.json", self.metadata.id.hyphenated()));
        self.save_to_file(&file_path).await?;
        Ok(())
    }

    pub async fn load_from_file<P: AsRef<Path>>(file_path: P) -> Result<Self, Error> {
        let text = fs::read(file_path).await?;
        let chat = serde_json::from_slice(&text)?;
        Ok(chat)
    }

    pub async fn save_to_file<P: AsRef<Path>>(&self, file_path: P) -> Result<(), Error> {
        fs::write(&file_path, serde_json::to_string_pretty(self)?).await?;
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    // TODO: Remove context information from errors, always log that at the
    // caller receiving the error. Getting chats failing in the API already
    // does.
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

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
