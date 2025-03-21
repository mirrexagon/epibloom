use std::sync::Mutex;

use rocket::{get, http::Status, response::status, serde::json::Json, State};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::chat::{Chat, Chats, Message, Role};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct NewChatResponse {
    chat_id: Uuid,
}

#[post("/")]
pub fn new_chat(chats: &State<Mutex<Chats>>) -> status::Created<Json<NewChatResponse>> {
    let mut chats = chats.lock().unwrap();

    let new_chat_id = chats.new_chat(None);

    status::Created::new(format!("/chat/{id}", id = new_chat_id)).body(Json(NewChatResponse {
        chat_id: new_chat_id,
    }))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetChatsResponseChat {
    chat_id: Uuid,
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetChatsResponse {
    chats: Vec<GetChatsResponseChat>,
}

#[get("/")]
pub fn get_chats(chats: &State<Mutex<Chats>>) -> Json<GetChatsResponse> {
    let chats = chats.lock().unwrap();

    let mut response = GetChatsResponse { chats: Vec::new() };

    for (chat_id, chat) in chats.chats().iter() {
        response.chats.push(GetChatsResponseChat {
            chat_id: chat_id.to_owned(),
            name: chat.name().to_owned(),
        });
    }

    Json(response)
}

#[get("/<chat_id>")]
pub fn get_chat_by_id(
    chats: &State<Mutex<Chats>>,
    chat_id: Uuid,
) -> Result<Json<Chat>, status::NotFound<()>> {
    let chats = chats.lock().unwrap();

    if let Some(chat) = chats.chats().get(&chat_id) {
        let chat = chat.clone();
        return Ok(Json(chat));
    }

    Err(status::NotFound(()))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushMessageRequest<'a> {
    role: Role,
    message: &'a str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushMessageResponse {
    message_index: usize,
}

#[post("/<chat_id>/message", data = "<message>")]
pub fn push_message(
    chats: &State<Mutex<Chats>>,
    chat_id: Uuid,
    message: Json<PushMessageRequest>,
) -> Result<Json<PushMessageResponse>, status::NotFound<()>> {
    let mut chats = chats.lock().unwrap();

    if let Some(chat) = chats.chats_mut().get_mut(&chat_id) {
        let new_message_index = chat.push_message(message.role, message.message);

        return Ok(Json(PushMessageResponse {
            message_index: new_message_index,
        }));
    }

    Err(status::NotFound(()))
}

//#[post("/<chat_id>/message/<message_index>")]
//pub fn update_message() {}
