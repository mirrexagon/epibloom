//! Internal chat representation and storage.

use std::fmt;

use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use time::macros::format_description;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::util;

// TODO: Assistant personas (characters) - chats can be associated with a
// character and inherit data from them

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("chat not found")]
    ChatNotFound(Uuid),
    #[error("database error: {0}")]
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
    #[serde(with = "time::serde::rfc3339")]
    pub created: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub modified: OffsetDateTime,
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

        let created = OffsetDateTime::from_unix_timestamp(row.get("created")?).unwrap();
        let modified = OffsetDateTime::from_unix_timestamp(row.get("modified")?).unwrap();

        Ok(Message {
            role,
            text,
            created,
            modified,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
        db_conn.execute("PRAGMA foreign_keys = ON", [])?;

        db_conn.execute(
            "CREATE TABLE IF NOT EXISTS chats (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                created INTEGER NOT NULL, -- Unix timestamp
                modified INTEGER NOT NULL -- Unix timestamp
            ) STRICT",
            [],
        )?;

        db_conn.execute(
            "CREATE TABLE IF NOT EXISTS messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                chat_id TEXT NOT NULL,
                position INTEGER NOT NULL,
                created INTEGER NOT NULL, -- Unix timestamp
                modified INTEGER NOT NULL, -- Unix timestamp
                role TEXT NOT NULL,
                text TEXT NOT NULL,
                FOREIGN KEY (chat_id) REFERENCES chats (id)
            ) STRICT",
            [],
        )?;

        Ok(Chats { db_conn })
    }

    pub fn list_chats(&self) -> Result<Vec<ChatMetadata>> {
        let mut statement = self
            .db_conn
            .prepare("SELECT id, name, created, modified FROM chats ORDER BY created DESC")?;

        let chats = statement.query_map([], ChatMetadata::from_row)?;
        chats
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(From::from)
    }

    fn chat_exists(&self, chat_id: Uuid) -> Result<bool> {
        self.db_conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM chats WHERE id = ?1);",
                [chat_id],
                |row| row.get(0).map(|v: bool| v),
            )
            .map_err(From::from)
    }

    /// Returns `None` if there is no chat with the given ID.
    fn get_chat_metadata_by_id(&self, chat_id: Uuid) -> Result<ChatMetadata> {
        let mut statement = self
            .db_conn
            .prepare("SELECT id, name, created, modified FROM chats WHERE id = ?1")?;

        match statement.query_row([chat_id], ChatMetadata::from_row) {
            Err(rusqlite::Error::QueryReturnedNoRows) => Err(Error::ChatNotFound(chat_id)),
            Ok(metadata) => Ok(metadata),

            // Note that `res` should always be `Err` at this point, so the
            // `map()` is just to satisfy the type checker.
            res => res.map_err(From::from),
        }
    }

    fn update_chat_last_modified_time(&self, chat_id: Uuid) -> Result<()> {
        let current_unix_timestamp = util::get_current_unix_timestamp();

        let num_rows_affected = self.db_conn.execute(
            "UPDATE chats SET modified = ?1 WHERE id = ?2",
            params![current_unix_timestamp, chat_id],
        )?;

        if num_rows_affected == 0 {
            return Err(Error::ChatNotFound(chat_id));
        }

        Ok(())
    }

    pub fn get_chat_by_id(&self, chat_id: Uuid) -> Result<Chat> {
        let metadata = self.get_chat_metadata_by_id(chat_id)?;

        let messages = {
            let mut statement = self.db_conn.prepare(
                "SELECT role, text, created, modified FROM messages WHERE chat_id = ?1 ORDER BY position ASC",
            )?;

            let messages = statement.query_map([chat_id], Message::from_row)?;
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

        let current_unix_timestamp = util::get_current_unix_timestamp();

        self.db_conn.execute(
            "INSERT INTO chats (id, name, created, modified) VALUES (?1, ?2, ?3, ?4)",
            params![
                new_chat_id,
                &new_chat_name,
                current_unix_timestamp,
                current_unix_timestamp
            ],
        )?;

        Ok(new_chat_id)
    }

    // TODO: Inserting a message in the middle:
    // https://softwareengineering.stackexchange.com/a/304597

    fn get_next_message_position_for_chat(&self, chat_id: Uuid) -> Result<usize> {
        if !self.chat_exists(chat_id)? {
            return Err(Error::ChatNotFound(chat_id));
        }

        match self.db_conn.query_row(
            "SELECT MAX(position) FROM messages WHERE id = ?1",
            [chat_id],
            |row| row.get(0),
        ) {
            Ok(Some(max_position)) => Ok(max_position),
            Ok(None) => Ok(0),
            Err(rusqlite::Error::QueryReturnedNoRows) => Err(Error::ChatNotFound(chat_id)),
            Err(err) => Err(Error::Database(err)),
        }
    }

    pub fn push_message(&self, chat_id: Uuid, role: Role, text: &str) -> Result<()> {
        // This will as a side effect confirm that the chat exists.
        let next_position = self.get_next_message_position_for_chat(chat_id)?;

        let current_unix_timestamp = util::get_current_unix_timestamp();

        self.db_conn.execute(
            "INSERT INTO messages (chat_id, position, created, modified, role, text) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![chat_id, next_position, current_unix_timestamp, current_unix_timestamp, role.to_string(), text],
        )?;

        self.update_chat_last_modified_time(chat_id)?;

        Ok(())
    }
}
