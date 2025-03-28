use std::sync::Mutex;

use rocket::{get, response::status, serde::json::Json, State};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::chat::{self, Chat, Chats, Role};

#[derive(Debug, Clone, Responder)]
pub enum ApiError {
    #[response(status = 404, content_type = "text")]
    NotFound(String),
    #[response(status = 500, content_type = "text")]
    DatabaseError(String),
}

impl From<chat::Error> for ApiError {
    fn from(value: chat::Error) -> Self {
        match value {
            chat::Error::ChatNotFound(chat_id) => {
                ApiError::NotFound(format!("chat not found: {chat_id}"))
            }
            chat::Error::Database(db_err) => {
                ApiError::DatabaseError(format!("database error: {}", db_err))
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct NewChatResponse {
    chat_id: Uuid,
}

#[post("/")]
pub fn new_chat(
    chats: &State<Mutex<Chats>>,
) -> Result<status::Created<Json<NewChatResponse>>, ApiError> {
    let chats = chats.lock().unwrap();

    let new_chat_id = chats.new_chat(None)?;

    Ok(
        status::Created::new(format!("/chat/{id}", id = new_chat_id)).body(Json(NewChatResponse {
            chat_id: new_chat_id,
        })),
    )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetChatsResponseChat {
    id: Uuid,
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetChatsResponse {
    chats: Vec<GetChatsResponseChat>,
}

#[get("/")]
pub fn get_chats(chats: &State<Mutex<Chats>>) -> Result<Json<GetChatsResponse>, ApiError> {
    let chats = chats.lock().unwrap();

    let chats_list = chats.list_chats()?;

    let mut response = GetChatsResponse { chats: Vec::new() };
    for chat_metadata in chats_list.iter() {
        response.chats.push(GetChatsResponseChat {
            id: chat_metadata.id,
            name: chat_metadata.name.to_owned(),
        });
    }

    Ok(Json(response))
}

#[get("/<chat_id>")]
pub fn get_chat_by_id(chats: &State<Mutex<Chats>>, chat_id: Uuid) -> Result<Json<Chat>, ApiError> {
    let chats = chats.lock().unwrap();

    chats.get_chat_by_id(chat_id).map(Json).map_err(From::from)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushMessageRequest<'a> {
    role: Role,
    text: &'a str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushMessageResponse {}

#[post("/<chat_id>/message", data = "<message>")]
pub fn push_message(
    chats: &State<Mutex<Chats>>,
    chat_id: Uuid,
    message: Json<PushMessageRequest>,
) -> Result<Json<PushMessageResponse>, ApiError> {
    let chats = chats.lock().unwrap();

    chats
        .push_message(chat_id, message.role, message.text)
        .map(|_| Json(PushMessageResponse {}))
        .map_err(From::from)
}

//#[post("/<chat_id>/message/<message_index>")]
//pub fn update_message() {}
