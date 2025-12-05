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

use super::{Chat, ChatMetadata, Error, Message, Role};

// TODO: Have every DB-connection-requiring function take a connection, and
// figure out how to get a rusqlite connection from what the pool gives.
// Probably derefs into it.
pub fn db_init(db_pool: &r2d2::Pool<SqliteConnectionManager>) -> Result<(), rusqlite::Error> {
    let db_conn = db_pool.get().unwrap();

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

pub(super) fn list_chats(
    db_pool: &r2d2::Pool<SqliteConnectionManager>,
) -> Result<Vec<ChatMetadata>, rusqlite::Error> {
    let db_conn = db_pool.get().unwrap();

    let mut statement =
        db_conn.prepare("SELECT id, name, created, modified FROM chats ORDER BY created DESC")?;

    let chats = statement.query_map([], ChatMetadata::from_row)?;
    chats.collect::<Result<Vec<_>, _>>().map_err(From::from)
}

fn chat_exists(
    db_pool: &r2d2::Pool<SqliteConnectionManager>,
    chat_id: Uuid,
) -> Result<bool, rusqlite::Error> {
    let db_conn = db_pool.get().unwrap();

    db_conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM chats WHERE id = ?1);",
        [chat_id],
        |row| row.get(0).map(|v: bool| v),
    )
}

/// Returns `None` if there is no chat with the given ID.
fn get_chat_metadata_by_id(
    db_pool: &r2d2::Pool<SqliteConnectionManager>,
    chat_id: Uuid,
) -> Result<ChatMetadata, Error> {
    let db_conn = db_pool.get().unwrap();

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

fn update_chat_last_modified_time(
    db_pool: &r2d2::Pool<SqliteConnectionManager>,
    chat_id: Uuid,
) -> Result<(), Error> {
    let current_unix_timestamp = util::get_current_unix_timestamp();

    let db_conn = db_pool.get().unwrap();
    let num_rows_affected = db_conn.execute(
        "UPDATE chats SET modified = ?1 WHERE id = ?2",
        params![current_unix_timestamp, chat_id],
    )?;

    if num_rows_affected == 0 {
        return Err(Error::ChatNotFound(chat_id));
    }

    Ok(())
}

pub(super) fn get_chat_by_id(
    db_pool: &r2d2::Pool<SqliteConnectionManager>,
    chat_id: Uuid,
) -> Result<Chat, Error> {
    let db_conn = db_pool.get().unwrap();

    let metadata = get_chat_metadata_by_id(db_pool, chat_id)?;

    let messages = {
        let mut statement = db_conn.prepare(
                "SELECT role, thinking, text, created, modified FROM chat_messages WHERE chat_id = ?1 ORDER BY position ASC",
            )?;

        let messages = statement.query_map([chat_id], Message::from_row)?;
        messages.collect::<Result<Vec<_>, _>>()?
    };

    Ok(Chat { metadata, messages })
}

/// Set `position` to `None` to get the last message.
pub(super) fn get_message(
    db_pool: &r2d2::Pool<SqliteConnectionManager>,
    chat_id: Uuid,
    position: Option<usize>,
) -> Result<Message, Error> {
    let position = get_message_position_default_latest(db_pool, chat_id, position)?;

    let db_conn = db_pool.get().unwrap();
    db_conn
            .query_row(
                "SELECT role, thinking, text, created, modified FROM chat_messages WHERE chat_id = ?1 AND position = ?2",
                params![chat_id, position],
                Message::from_row,
            )
            .map_err(From::from)

    // TODO: use Error::MessageNotFound when no rows returned
}

fn get_message_position_default_latest(
    db_pool: &r2d2::Pool<SqliteConnectionManager>,
    chat_id: Uuid,
    position: Option<usize>,
) -> Result<usize, Error> {
    if !chat_exists(db_pool, chat_id)? {
        return Err(Error::ChatNotFound(chat_id));
    }

    if let Some(position) = position {
        Ok(position)
    } else {
        get_latest_message_position(db_pool, chat_id)
    }
}

pub(super) fn new_chat(
    db_pool: &r2d2::Pool<SqliteConnectionManager>,
    name: Option<&str>,
) -> Result<ChatMetadata, Error> {
    let mut new_chat_id;

    loop {
        new_chat_id = Uuid::new_v4();

        match get_chat_metadata_by_id(db_pool, new_chat_id) {
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

    let db_conn = db_pool.get().unwrap();
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

fn get_latest_message_position(
    db_pool: &r2d2::Pool<SqliteConnectionManager>,
    chat_id: Uuid,
) -> Result<usize, Error> {
    let db_conn = db_pool.get().unwrap();

    if !chat_exists(db_pool, chat_id)? {
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

pub(super) fn push_message(
    db_pool: &r2d2::Pool<SqliteConnectionManager>,
    chat_id: Uuid,
    role: Role,
    thinking: Option<&str>,
    text: &str,
) -> Result<(), Error> {
    if thinking.is_some() && role != Role::Assistant {
        return Err(Error::ThinkingSuppliedForNonAssistantMessage);
    }

    // This will as a side effect confirm that the chat exists.
    let next_position = match get_latest_message_position(db_pool, chat_id) {
        Ok(position) => position + 1,
        Err(Error::ChatHasNoMessages(_)) => 0,
        Err(err) => return Err(err),
    };

    let current_unix_timestamp = util::get_current_unix_timestamp();

    let db_conn = db_pool.get().unwrap();
    db_conn.execute(
            "INSERT INTO chat_messages (chat_id, position, created, modified, role, thinking, text) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![chat_id, next_position, current_unix_timestamp, current_unix_timestamp, role.to_string(), thinking, text],
        )?;

    update_chat_last_modified_time(db_pool, chat_id)?;

    Ok(())
}

/// Set `position` to `None` to update the last message.
pub(super) fn update_message(
    db_pool: &r2d2::Pool<SqliteConnectionManager>,
    chat_id: Uuid,
    position: Option<usize>,
    text: &str,
) -> Result<(), Error> {
    let current_unix_timestamp = util::get_current_unix_timestamp();

    let position = get_message_position_default_latest(db_pool, chat_id, position)?;

    let db_conn = db_pool.get().unwrap();
    db_conn.execute(
        "UPDATE chat_messages SET modified = ?1, text = ?2 WHERE chat_id = ?3 AND position = ?4",
        params![current_unix_timestamp, text, chat_id, position],
    )?;

    update_chat_last_modified_time(db_pool, chat_id)?;

    Ok(())
}

impl Message {
    fn from_row(row: &Row) -> Result<Self, rusqlite::Error> {
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

impl ChatMetadata {
    fn from_row(row: &Row) -> Result<Self, rusqlite::Error> {
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
