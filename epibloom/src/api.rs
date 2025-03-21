use std::sync::Mutex;

use rocket::serde::{Deserialize, Serialize};
use rocket::{get, http::Status, response::status, serde::json::Json, State};
use serde_json::json;
use time::macros::format_description;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::anthropic;
use crate::chat::{Chat, Chats};

// TODO: Move Rocket-agnostic logic to impl functions on the chat structs.

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct NewChatResponse {
    id: Uuid,
}

#[post("/chat")]
pub fn new_chat(chats: &State<Mutex<Chats>>) -> status::Created<Json<NewChatResponse>> {
    let mut chats = chats.lock().unwrap();

    let new_chat_id = chats.new_chat(None);

    status::Created::new(format!("/chat/{id}", id = new_chat_id))
        .body(Json(NewChatResponse { id: new_chat_id }))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct GetChatsResponseChat {
    id: Uuid,
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct GetChatsResponse {
    chats: Vec<GetChatsResponseChat>,
}

#[get("/chat")]
pub fn get_chats(chats: &State<Mutex<Chats>>) -> Json<GetChatsResponse> {
    let chats = chats.lock().unwrap();

    let mut response = GetChatsResponse { chats: Vec::new() };

    for (chat_id, chat) in chats.chats().iter() {
        response.chats.push(GetChatsResponseChat {
            id: chat_id.to_owned(),
            name: chat.name().to_owned(),
        });
    }

    Json(response)
}

#[get("/chat/<id>")]
pub fn get_chat_by_id(
    chats: &State<Mutex<Chats>>,
    id: Uuid,
) -> Result<Json<Chat>, status::NotFound<()>> {
    let chats = chats.lock().unwrap();

    if let Some(chat) = chats.chats().get(&id) {
        let chat = chat.clone();
        return Ok(Json(chat));
    }

    Err(status::NotFound(()))
}

#[get("/anthropic/models")]
pub async fn list_anthropic_models(
    anthropic_api: &State<anthropic::AnthropicApi>,
) -> Result<Json<Vec<anthropic::ModelInfo>>, status::Custom<String>> {
    let model_list = anthropic_api.list_models().await.map_err(|err| {
        status::Custom(
            Status::BadGateway,
            format!("Failed to get model list from Anthropic: {err}"),
        )
    })?;

    Ok(Json(model_list))
}

//#[post("/chat/<id>/message")]
//pub fn add_message() {}
