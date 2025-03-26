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
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMetadata {
    id: Uuid,
    name: String,
}

impl ChatMetadata {
    fn from_row(row: &Row) -> Result<Self> {
        let id: String = row.get(0)?;
        let name = row.get(1)?;

        Ok(ChatMetadata {
            id: Uuid::parse_str(&id).unwrap(),
            name,
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
                message TEXT NOT NULL,
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

        /*
        let mut rows = statement.query([])?;
        let mut chats = Vec::new();
        while let Some(row) = rows.next()? {
            let id: String = row.get(0)?;
            let name = row.get(1)?;

            chats.push(ChatMetadata {
                id: Uuid::parse_str(&id).unwrap(),
                name,
            });
        }*/

        let chats = statement.query_map([], ChatMetadata::from_row)?;
        let chats = chats.collect::<Result<Vec<_>, _>>()?;
        Ok(chats)
    }

    pub fn get_chat_by_id(&self, id: Uuid) -> Result<Chat> {
        let chat_metadata = {
            let mut statement = self
                .db_conn
                .prepare("SELECT id, name FROM chats WHERE id = ?1")?;

            statement.query_row([&id.to_string()], ChatMetadata::from_row)?
        };

        let chat_messages = {
            let mut statement = self
                .db_conn
                .prepare("SELECT role, message FROM messages WHERE chat_id = ?1 ORDER BY")?;
        };
    }

    pub fn new_chat(&mut self, name: Option<&str>) -> Uuid {
        let new_chat_id = Uuid::new_v4();

        assert!(!self.chats.contains_key(&new_chat_id));

        let new_chat_name = if let Some(name) = name {
            name.to_owned()
        } else {
            let current_datetime = OffsetDateTime::now_local().unwrap();
            let dtfmt =
                format_description!(version = 2, "[year]-[month]-[day]T[hour][minute][second]");
            current_datetime.format(&dtfmt).unwrap()
        };

        self.chats.insert(new_chat_id, Chat::new(&new_chat_name));

        new_chat_id
    }
}
