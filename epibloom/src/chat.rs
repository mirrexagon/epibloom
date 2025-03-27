use std::fmt;

use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use time::macros::format_description;
use time::OffsetDateTime;
use uuid::Uuid;

// TODO: Assistant personas (characters) - chats can be associated with a
// character and inherit data from them

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("chat not found")]
    ChatNotFound(Uuid),
    #[error("database error")]
    Database(#[from] rusqlite::Error),
}

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum Role {
    #[serde(rename = "assistant")]
    Assistant,
    #[serde(rename = "user")]
    User,
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match *self {
            Role::Assistant => "assistant",
            Role::User => "user",
        };

        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub text: String,
}

impl Message {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        let role: String = row.get("role")?;
        let role = match role.as_str() {
            "assistant" => Role::Assistant,
            "user" => Role::User,
            r => panic!("Unknown role in database: {}", r),
        };

        let text = row.get("text")?;

        Ok(Message { role, text })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMetadata {
    pub id: Uuid,
    pub name: String,
}

impl ChatMetadata {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        let id: String = row.get("id")?;
        let id = Uuid::parse_str(&id).unwrap();
        let name = row.get("name")?;

        Ok(ChatMetadata { id, name })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chat {
    #[serde(flatten)]
    pub metadata: ChatMetadata,
    pub messages: Vec<Message>,
}

#[derive(Debug)]
pub struct Chats {
    db_conn: Connection,
}

impl Chats {
    pub fn new(db_conn: Connection) -> Result<Self> {
        db_conn.execute(
            "CREATE TABLE IF NOT EXISTS chats (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL
            )",
            [],
        )?;

        db_conn.execute(
            "CREATE TABLE IF NOT EXISTS messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                chat_id TEXT NOT NULL,
                position INTEGER NOT NULL,
                role TEXT NOT NULL,
                text TEXT NOT NULL,
                FOREIGN KEY (chat_id) REFERENCES chats (id)
            )",
            [],
        )?;

        Ok(Chats { db_conn })
    }

    pub fn list_chats(&self) -> Result<Vec<ChatMetadata>> {
        let mut statement = self
            .db_conn
            .prepare("SELECT id, name FROM chats ORDER BY name")?;

        let chats = statement.query_map([], ChatMetadata::from_row)?;
        chats
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(From::from)
    }

    /// Returns `None` if there is no chat with the given ID.
    fn get_chat_metadata_by_id(&self, id: Uuid) -> Result<ChatMetadata> {
        let mut statement = self
            .db_conn
            .prepare("SELECT id, name FROM chats WHERE id = ?1")?;

        match statement.query_row([id], ChatMetadata::from_row) {
            Err(rusqlite::Error::QueryReturnedNoRows) => Err(Error::ChatNotFound(id)),
            Ok(metadata) => Ok(metadata),

            // Note that `res` should always be `Err` at this point, so the
            // `map()` is just to satisfy the type checker.
            res => res.map_err(From::from),
        }
    }

    pub fn get_chat_by_id(&self, id: Uuid) -> Result<Chat> {
        let metadata = self.get_chat_metadata_by_id(id)?;

        let messages = {
            let mut statement = self.db_conn.prepare(
                "SELECT role, message FROM messages WHERE chat_id = ?1 ORDER BY position ASC",
            )?;

            let messages = statement.query_map([id], Message::from_row)?;
            messages.collect::<std::result::Result<Vec<_>, _>>()?
        };

        Ok(Chat { metadata, messages })
    }

    pub fn new_chat(&self, name: Option<&str>) -> Result<Uuid> {
        let mut new_chat_id;

        loop {
            new_chat_id = Uuid::new_v4();

            match self.get_chat_metadata_by_id(new_chat_id) {
                Err(Error::ChatNotFound(_)) => break,
                Ok(_) => continue,

                // Note that `res` should always be `Err` at this point, so the
                // `map()` is just to satisfy the type checker.
                res => return res.map(|chat_metadata| chat_metadata.id),
            }
        }

        let new_chat_name = if let Some(name) = name {
            name.to_owned()
        } else {
            let current_datetime = OffsetDateTime::now_local().unwrap();
            let dtfmt =
                format_description!(version = 2, "[year]-[month]-[day]T[hour][minute][second]");
            current_datetime.format(&dtfmt).unwrap()
        };

        self.db_conn.execute(
            "INSERT INTO chats (id, name) VALUES (?1, ?2)",
            params![new_chat_id, &new_chat_name],
        )?;

        Ok(new_chat_id)
    }

    // TODO: Inserting a message in the middle:
    // https://softwareengineering.stackexchange.com/a/304597

    fn get_next_message_position_for_chat(&self, id: Uuid) -> Result<usize> {
        // Just to easily error with the correct error if the chat doesn't exist.
        let metadata = self.get_chat_metadata_by_id(id)?;

        match self.db_conn.query_row(
            "SELECT MAX(position) FROM messages WHERE id = ?1",
            [id],
            |row| row.get(0),
        ) {
            Ok(max_position) => Ok(max_position),
            Err(rusqlite::Error::QueryReturnedNoRows) => Err(Error::ChatNotFound(id)),
            Err(err) => Err(Error::Database(err)),
        }
    }

    pub fn push_message(&self, chat_id: Uuid, role: Role, text: &str) -> Result<()> {
        // This will as a side effect confirm that the chat exists.
        let next_position = self.get_next_message_position_for_chat(chat_id)?;

        self.db_conn.execute(
            "INSERT INTO messages (chat_id, position, role, text) VALUES (?1, ?2, ?3, ?4)",
            params![chat_id, next_position, role.to_string(), text],
        )?;

        Ok(())
    }
}
