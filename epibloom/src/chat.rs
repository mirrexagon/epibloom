use std::collections::HashMap;

use rusqlite::{Connection, Result, Row};
use serde::{Deserialize, Serialize};
use time::macros::format_description;
use time::OffsetDateTime;
use uuid::Uuid;

// TODO: Assistant personas (characters) - chats can be associated with a
// character and inherit data from them

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum Role {
    #[serde(rename = "assistant")]
    Assistant,
    #[serde(rename = "user")]
    User,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub text: String,
}

impl Message {
    fn from_row(row: &Row) -> Result<Self> {
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
    id: Uuid,
    name: String,
}

impl ChatMetadata {
    fn from_row(row: &Row) -> Result<Self> {
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

    // TODO: Inserting a message in the middle:
    // https://softwareengineering.stackexchange.com/a/304597

    pub fn list_chats(&self) -> Result<Vec<ChatMetadata>> {
        let mut statement = self
            .db_conn
            .prepare("SELECT id, name FROM chats ORDER BY name")?;

        let chats = statement.query_map([], ChatMetadata::from_row)?;
        chats.collect::<Result<Vec<_>, _>>()
    }

    pub fn get_chat_by_id(&self, id: Uuid) -> Result<Chat> {
        let id_string = id.to_string();

        let metadata = {
            let mut statement = self
                .db_conn
                .prepare("SELECT id, name FROM chats WHERE id = ?1")?;

            statement.query_row([&id_string], ChatMetadata::from_row)?
        };

        let messages = {
            let mut statement = self.db_conn.prepare(
                "SELECT role, message FROM messages WHERE chat_id = ?1 ORDER BY position ASC",
            )?;

            let messages = statement.query_map([&id_string], Message::from_row)?;
            messages.collect::<Result<Vec<_>, _>>()?
        };

        Ok(Chat { metadata, messages })
    }

    pub fn new_chat(&self, name: Option<&str>) -> Result<Uuid> {
        let mut new_chat_id;

        loop {
            new_chat_id = Uuid::new_v4();

            match self.get_chat_by_id(new_chat_id) {
                Err(rusqlite::Error::QueryReturnedNoRows) => break,
                Ok(_) => continue,

                // Note that `res` should always be `Err` at this point, so the
                // `map()` is just to satisfy the type checker.
                res => return res.map(|chat| chat.metadata.id),
            }
        }

        let new_chat_id_string = new_chat_id.to_string();

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
            [&new_chat_id_string, &new_chat_name],
        )?;

        Ok(new_chat_id)
    }
}
