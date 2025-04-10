use std::sync::Mutex;

use rocket::{get, response::status, serde::json::Json, State};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    backends::anthropic::{self, AnthropicApi},
    backends::Backend,
    data::chat::{self, Chat, ChatMetadata, Chats, Role},
};

#[derive(Debug, Clone, Responder)]
pub enum ApiError {
    #[response(status = 404, content_type = "text")]
    NotFound(String),
    #[response(status = 500, content_type = "text")]
    Database(String),
    #[response(status = 502, content_type = "text")]
    Backend(String),
}

impl From<chat::Error> for ApiError {
    fn from(value: chat::Error) -> Self {
        match value {
            chat::Error::ChatNotFound(_)
            | chat::Error::ChatHasNoMessages(_)
            | chat::Error::MessageNotFound { .. } => ApiError::NotFound(value.to_string()),
            chat::Error::Database(_) => ApiError::Database(value.to_string()),
        }
    }
}

impl From<anthropic::Error> for ApiError {
    fn from(value: anthropic::Error) -> Self {
        ApiError::Backend(value.to_string())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct NewChatResponse {
    chat_id: Uuid,
}

#[post("/")]
pub fn new_chat(
    chats: &State<Mutex<Chats>>,
) -> Result<status::Created<Json<ChatMetadata>>, ApiError> {
    let chats = chats.lock().unwrap();

    let new_chat_metadata = chats.new_chat(None)?;

    Ok(
        status::Created::new(format!("/chat/{id}", id = new_chat_metadata.id))
            .body(Json(new_chat_metadata)),
    )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetChatsResponse {
    chats: Vec<chat::ChatMetadata>,
}

#[get("/")]
pub fn get_chats(chats: &State<Mutex<Chats>>) -> Result<Json<GetChatsResponse>, ApiError> {
    let chats = chats.lock().unwrap();

    let chats_list = chats.list_chats()?;

    let mut response = GetChatsResponse { chats: Vec::new() };
    for chat_metadata in chats_list.iter() {
        response.chats.push(chat_metadata.clone());
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateResponse {
    message: String,
}

#[post("/<chat_id>/generate?<continue>")]
pub async fn generate(
    chats: &State<Mutex<Chats>>,
    anthropic_api: &State<AnthropicApi>,
    chat_id: Uuid,
    r#continue: bool,
) -> Result<Json<GenerateResponse>, ApiError> {
    let chat = {
        let chats = chats.lock().unwrap();
        chats.get_chat_by_id(chat_id)?
    };

    let anthropic_config = anthropic::Config::default();

    // TODO for continue: Anthropic API returned error 'invalid_request_error': messages: final assistant content cannot end with trailing whitespace
    let mut new_message = anthropic_api.generate(&anthropic_config, &chat).await?;

    {
        let chats = chats.lock().unwrap();

        if r#continue {
            let mut combined_message = chats.get_message(chat_id, None)?.text;
            combined_message.push_str(&new_message);
            chats.update_message(chat_id, None, &combined_message)?;
            new_message = combined_message;
        } else {
            chats.push_message(chat_id, Role::Assistant, &new_message)?;
        }
    }

    Ok(Json(GenerateResponse {
        message: new_message,
    }))
}
