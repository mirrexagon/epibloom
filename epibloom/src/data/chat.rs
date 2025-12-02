use std::{collections::HashMap, fmt, sync::Arc};

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

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("chat not found: {0}")]
    ChatNotFound(Uuid),

    #[error("chat {0} has no messages")]
    ChatHasNoMessages(Uuid),

    #[error("message not found: chat {chat_id}, message position {position}")]
    MessageNotFound { chat_id: Uuid, position: usize },

    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
}

type Result<T> = std::result::Result<T, Error>;

pub struct ChatSessionStore {
    sessions: Arc<RwLock<HashMap<Uuid, Arc<RwLock<ChatSession>>>>>,
}

impl ChatSessionStore {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Returns `None` if there is no chat with the given ID.
    pub async fn get_or_create_session(
        &self,
        db_conn: &Connection,
        chat_id: Uuid,
    ) -> Option<Arc<RwLock<ChatSession>>> {
        let mut sessions = self.sessions.write().await;

        if let Ok(chat) = get_chat_by_id(db_conn, chat_id) {
            let (broadcast, _) = broadcast::channel(1024);

            Some(
                sessions
                    .entry(chat_id)
                    .or_insert_with(|| {
                        Arc::new(RwLock::new(ChatSession {
                            chat,
                            current_assistant_message: String::new(),
                            current_assistant_thinking: String::new(),
                            broadcast,
                            state: ChatSessionState::Idle,
                        }))
                    })
                    .clone(),
            )
        } else {
            None
        }
    }

    // TODO: Removing sessions when no clients are active and no generation is happening.
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatSessionEvent {
    UserMessageStart { position: usize },
    UserMessageDelta { delta: String },
    UserMessageEnd { position: usize },
    AssistantMessageStart { position: usize },
    AssistantThinkingDelta { delta: String },
    AssistantMessageDelta { delta: String },
    AssistantMessageEnd { position: usize },
    Error { message: String },
}

pub enum ChatSessionState {
    Idle,
    Generating,
}

/// Live chat streaming sessions created on demand when either starting
/// generation or requesting a session. These are used mainly to be the single
/// source of streaming generation for the chat.
pub struct ChatSession {
    chat: Chat,

    current_assistant_thinking: String,
    /// Current incomplete assistant message. Will be added to the chat once it is complete.
    current_assistant_message: String,

    broadcast: broadcast::Sender<ChatSessionEvent>,
    state: ChatSessionState,
}

impl ChatSession {
    pub fn get_new_receiver(&self) -> broadcast::Receiver<ChatSessionEvent> {
        self.broadcast.subscribe()
    }

    pub fn get_complete_messages(&self) -> &[Message] {
        &self.chat.messages[..]
    }

    pub async fn start_generating<B: backends::Backend + Send + Sync + 'static>(
        session_arc: Arc<RwLock<ChatSession>>,
        backend: Arc<B>,
        backend_config: Arc<B::Config>,
    ) where
        <B as backends::Backend>::Error: Send + Sync + 'static,
        <B as backends::Backend>::Config: Send + Sync + 'static,
    {
        let chat = {
            let mut session = session_arc.write().await;
            session.state = ChatSessionState::Generating;
            session.chat.clone()
        };

        tokio::spawn(async move {
            let stream = backend.generate(&backend_config, &chat);
            process_generation_stream(session_arc.clone(), stream, chat.messages.len())
                .await
                .unwrap();

            // Stream has completed.
            {
                let mut session = session_arc.write().await;

                // TODO: Push message to database
                // TODO: Push thinking
                /*
                    data::chat::push_message(
                        db_conn,
                        session.chat.metadata.id,
                        Role::Assistant,
                        &session.current_assistant_message,
                    )
                    .unwrap();
                */

                let new_thinking = session.current_assistant_thinking.clone();
                let new_message = session.current_assistant_message.clone();

                session.chat.messages.push(Message {
                    role: Role::Assistant,
                    thinking: new_thinking,
                    text: new_message,
                    created: time::UtcDateTime::now().into(),
                    modified: time::UtcDateTime::now().into(),
                });

                session.current_assistant_message.truncate(0);
                session.current_assistant_thinking.truncate(0);

                session.state = ChatSessionState::Idle;
            }
        });
    }
}

async fn process_generation_stream<S, E>(
    session_arc: Arc<RwLock<ChatSession>>,
    stream: S,
    message_position: usize,
) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    S: futures::Stream<Item = std::result::Result<backends::GenerateChunk, E>>,
    E: std::error::Error + Send + Sync + 'static,
{
    use futures::stream::StreamExt;

    futures::pin_mut!(stream);

    {
        let session = session_arc.write().await;
        let _ = session
            .broadcast
            .send(ChatSessionEvent::AssistantMessageStart {
                position: message_position,
            });
    }

    while let Some(result) = stream.next().await {
        let mut session = session_arc.write().await;
        match result {
            Ok(chunk) => match chunk {
                backends::GenerateChunk::ThinkingDelta(ref delta) => {
                    session.current_assistant_thinking.push_str(delta);

                    // Ignore failure to send as there may be no receivers.
                    let _ = session
                        .broadcast
                        .send(ChatSessionEvent::AssistantThinkingDelta {
                            delta: delta.clone(),
                        });
                }
                backends::GenerateChunk::MessageDelta(ref delta) => {
                    session.current_assistant_message.push_str(delta);
                    let _ = session
                        .broadcast
                        .send(ChatSessionEvent::AssistantMessageDelta {
                            delta: delta.clone(),
                        });
                }
            },
            Err(err) => {
                let _ = session.broadcast.send(ChatSessionEvent::Error {
                    message: err.to_string(),
                });
                return Err(Box::new(err));
            }
        }
    }

    {
        let session = session_arc.write().await;
        let _ = session
            .broadcast
            .send(ChatSessionEvent::AssistantMessageEnd {
                position: message_position,
            });
    }

    Ok(())
}

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
    pub thinking: String,
    pub text: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub modified: OffsetDateTime,
}

impl Message {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        let role: String = row.get("role")?;
        let role = match role.as_str() {
            "system" => Role::System,
            "assistant" => Role::Assistant,
            "user" => Role::User,
            r => panic!("Unknown role in database: {r}"),
        };

        let thinking = row.get("thinking")?;
        let text = row.get("text")?;

        let created = OffsetDateTime::from_unix_timestamp(row.get("created")?).unwrap();
        let modified = OffsetDateTime::from_unix_timestamp(row.get("modified")?).unwrap();

        Ok(Message {
            role,
            thinking,
            text,
            created,
            modified,
        })
    }
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
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        let id: Uuid = row.get("id")?;
        let name = row.get("name")?;
        let created = OffsetDateTime::from_unix_timestamp(row.get("created")?).unwrap();
        let modified = OffsetDateTime::from_unix_timestamp(row.get("modified")?).unwrap();

        Ok(ChatMetadata {
            id,
            name,
            created,
            modified,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Chat {
    #[serde(flatten)]
    pub metadata: ChatMetadata,

    pub messages: Vec<Message>,
}

pub fn db_init(db_conn: &Connection) -> Result<()> {
    db_conn.execute("PRAGMA foreign_keys = ON", [])?;

    db_conn.execute(
        "CREATE TABLE IF NOT EXISTS chats (
                id BLOB PRIMARY KEY,
                name TEXT NOT NULL,
                created INTEGER NOT NULL, -- Unix timestamp
                modified INTEGER NOT NULL -- Unix timestamp
            ) STRICT",
        [],
    )?;

    db_conn.execute(
        "CREATE TABLE IF NOT EXISTS chat_messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                chat_id BLOB NOT NULL,
                position INTEGER NOT NULL,
                created INTEGER NOT NULL, -- Unix timestamp
                modified INTEGER NOT NULL, -- Unix timestamp
                role TEXT NOT NULL,
                thinking TEXT,
                text TEXT NOT NULL,
                FOREIGN KEY (chat_id) REFERENCES chats (id)
            ) STRICT",
        [],
    )?;

    Ok(())
}

pub fn list_chats(db_conn: &Connection) -> Result<Vec<ChatMetadata>> {
    let mut statement =
        db_conn.prepare("SELECT id, name, created, modified FROM chats ORDER BY created DESC")?;

    let chats = statement.query_map([], ChatMetadata::from_row)?;
    chats
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(From::from)
}

fn chat_exists(db_conn: &Connection, chat_id: Uuid) -> Result<bool> {
    db_conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM chats WHERE id = ?1);",
            [chat_id],
            |row| row.get(0).map(|v: bool| v),
        )
        .map_err(From::from)
}

/// Returns `None` if there is no chat with the given ID.
fn get_chat_metadata_by_id(db_conn: &Connection, chat_id: Uuid) -> Result<ChatMetadata> {
    let mut statement =
        db_conn.prepare("SELECT id, name, created, modified FROM chats WHERE id = ?1")?;

    match statement.query_row([chat_id], ChatMetadata::from_row) {
        Err(rusqlite::Error::QueryReturnedNoRows) => Err(Error::ChatNotFound(chat_id)),
        Ok(metadata) => Ok(metadata),

        // Note that `res` should always be `Err` at this point, so the
        // `map()` is just to satisfy the type checker.
        res => res.map_err(From::from),
    }
}

fn update_chat_last_modified_time(db_conn: &Connection, chat_id: Uuid) -> Result<()> {
    let current_unix_timestamp = util::get_current_unix_timestamp();

    let num_rows_affected = db_conn.execute(
        "UPDATE chats SET modified = ?1 WHERE id = ?2",
        params![current_unix_timestamp, chat_id],
    )?;

    if num_rows_affected == 0 {
        return Err(Error::ChatNotFound(chat_id));
    }

    Ok(())
}

pub fn get_chat_by_id(db_conn: &Connection, chat_id: Uuid) -> Result<Chat> {
    let metadata = get_chat_metadata_by_id(db_conn, chat_id)?;

    let messages = {
        let mut statement = db_conn.prepare(
                "SELECT role, text, created, modified FROM chat_messages WHERE chat_id = ?1 ORDER BY position ASC",
            )?;

        let messages = statement.query_map([chat_id], Message::from_row)?;
        messages.collect::<std::result::Result<Vec<_>, _>>()?
    };

    Ok(Chat { metadata, messages })
}

/// Set `position` to `None` to get the last message.
pub fn get_message(
    db_conn: &Connection,
    chat_id: Uuid,
    position: Option<usize>,
) -> Result<Message> {
    let position = get_message_position_default_latest(db_conn, chat_id, position)?;

    db_conn
            .query_row(
                "SELECT role, text, created, modified FROM chat_messages WHERE chat_id = ?1 AND position = ?2",
                params![chat_id, position],
                Message::from_row,
            )
            .map_err(From::from)

    // TODO: use Error::MessageNotFound when no rows returned
}

fn get_message_position_default_latest(
    db_conn: &Connection,
    chat_id: Uuid,
    position: Option<usize>,
) -> Result<usize> {
    if !chat_exists(db_conn, chat_id)? {
        return Err(Error::ChatNotFound(chat_id));
    }

    if let Some(position) = position {
        Ok(position)
    } else {
        get_latest_message_position(db_conn, chat_id)
    }
}

pub fn new_chat(db_conn: &Connection, name: Option<&str>) -> Result<ChatMetadata> {
    let mut new_chat_id;

    loop {
        new_chat_id = Uuid::new_v4();

        match get_chat_metadata_by_id(db_conn, new_chat_id) {
            Err(Error::ChatNotFound(_)) => break,
            Ok(_) => continue,

            // Note that `res` should always be `Err` at this point, so the
            // `map()` is just to satisfy the type checker.
            res => return res,
        }
    }

    let current_datetime = OffsetDateTime::now_local().unwrap();

    let new_chat_name = if let Some(name) = name {
        name.to_owned()
    } else {
        let dtfmt = format_description!(version = 2, "[year]-[month]-[day]T[hour][minute][second]");
        current_datetime.format(&dtfmt).unwrap()
    };

    let current_unix_timestamp = current_datetime.unix_timestamp();

    db_conn.execute(
        "INSERT INTO chats (id, name, created, modified) VALUES (?1, ?2, ?3, ?4)",
        params![
            new_chat_id,
            &new_chat_name,
            current_unix_timestamp,
            current_unix_timestamp
        ],
    )?;

    Ok(ChatMetadata {
        id: new_chat_id,
        name: new_chat_name,
        created: current_datetime,
        modified: current_datetime,
    })
}

// TODO: Inserting a message in the middle:
// https://softwareengineering.stackexchange.com/a/304597

fn get_latest_message_position(db_conn: &Connection, chat_id: Uuid) -> Result<usize> {
    if !chat_exists(db_conn, chat_id)? {
        return Err(Error::ChatNotFound(chat_id));
    }

    match db_conn.query_row(
        "SELECT MAX(position) FROM chat_messages WHERE chat_id = ?1",
        [chat_id],
        |row| row.get(0),
    ) {
        Ok(Some(latest_message_position)) => Ok(latest_message_position),
        Ok(None) => Err(Error::ChatHasNoMessages(chat_id)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Err(Error::ChatHasNoMessages(chat_id)),
        Err(err) => Err(Error::Database(err)),
    }
}

pub fn push_message(db_conn: &Connection, chat_id: Uuid, role: Role, text: &str) -> Result<()> {
    // This will as a side effect confirm that the chat exists.
    let next_position = match get_latest_message_position(db_conn, chat_id) {
        Ok(position) => position + 1,
        Err(Error::ChatHasNoMessages(_)) => 0,
        Err(err) => return Err(err),
    };

    let current_unix_timestamp = util::get_current_unix_timestamp();

    db_conn.execute(
            "INSERT INTO chat_messages (chat_id, position, created, modified, role, text) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![chat_id, next_position, current_unix_timestamp, current_unix_timestamp, role.to_string(), text],
        )?;

    update_chat_last_modified_time(db_conn, chat_id)?;

    Ok(())
}

/// Set `position` to `None` to update the last message.
pub fn update_message(
    db_conn: &Connection,
    chat_id: Uuid,
    position: Option<usize>,
    text: &str,
) -> Result<()> {
    let current_unix_timestamp = util::get_current_unix_timestamp();

    let position = get_message_position_default_latest(db_conn, chat_id, position)?;

    db_conn.execute(
        "UPDATE chat_messages SET modified = ?1, text = ?2 WHERE chat_id = ?3 AND position = ?4",
        params![current_unix_timestamp, text, chat_id, position],
    )?;

    update_chat_last_modified_time(db_conn, chat_id)?;

    Ok(())
}
