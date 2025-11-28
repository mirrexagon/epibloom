use std::sync::Mutex;

use rocket::{get, response::status, serde::json::Json, State};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, OpenApi, ToSchema};
use uuid::Uuid;

use crate::{
    backends::{
        anthropic::{self, AnthropicApi},
        Backend,
    },
    data::chat::{self, Chat, ChatMetadata, Role},
};

use super::ApiErrorResponse;

impl From<chat::Error> for ApiErrorResponse {
    fn from(value: chat::Error) -> Self {
        match value {
            chat::Error::ChatNotFound(_)
            | chat::Error::ChatHasNoMessages(_)
            | chat::Error::MessageNotFound { .. } => ApiErrorResponse::NotFound(value.to_string()),
            chat::Error::Database(_) => ApiErrorResponse::InternalServerError(value.to_string()),
        }
    }
}

#[derive(OpenApi)]
#[openapi(paths(
    new_chat,
    get_chats,
    get_chat_by_id,
    get_rendered_chat_by_id,
    push_message,
    generate
))]
pub struct ChatApiDoc;

/// Creates a new chat.
#[utoipa::path(
    responses(
        (status = 201, description = "Chat created successfully.", body = ChatMetadata),
        (status = 500, description = "Database operation failed.", body = String)
    )
)]
#[post("/")]
pub fn new_chat(
    db_conn: &State<Mutex<rusqlite::Connection>>,
) -> Result<status::Created<Json<ChatMetadata>>, ApiErrorResponse> {
    let db_conn = db_conn.lock().unwrap();

    let new_chat_metadata = chat::new_chat(&db_conn, None)?;

    Ok(
        status::Created::new(format!("/chat/{id}", id = new_chat_metadata.id))
            .body(Json(new_chat_metadata)),
    )
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct GetChatsResponse {
    chats: Vec<chat::ChatMetadata>,
}

/// Gets a list of all chats in the database.
///
/// Just includes metadata, no messages.
#[utoipa::path(
    responses(
        (status = 200, description = "Got chat list succesfully.", body = GetChatsResponse),
        (status = 500, description = "Database operation failed.", body = String)
    )
)]
#[get("/")]
pub fn get_chats(
    db_conn: &State<Mutex<rusqlite::Connection>>,
) -> Result<Json<GetChatsResponse>, ApiErrorResponse> {
    let db_conn = db_conn.lock().unwrap();

    let chats_list = chat::list_chats(&db_conn)?;

    let mut response = GetChatsResponse { chats: Vec::new() };
    for chat_metadata in chats_list.iter() {
        response.chats.push(chat_metadata.clone());
    }

    Ok(Json(response))
}

/// Gets a single chat by ID.
///
/// Includes chat metadata and all messages.
#[utoipa::path(
    responses(
        (status = 200, description = "Got chat succesfully.", body = Chat),
        (status = 500, description = "Database operation failed.", body = String)
    )
)]
#[get("/<chat_id>")]
pub fn get_chat_by_id(
    db_conn: &State<Mutex<rusqlite::Connection>>,
    chat_id: Uuid,
) -> Result<Json<Chat>, ApiErrorResponse> {
    let db_conn = db_conn.lock().unwrap();

    chat::get_chat_by_id(&db_conn, chat_id)
        .map(Json)
        .map_err(From::from)
}

/// Gets a rendered chat by ID.
///
/// Returns the chat in the simplified RenderedChat format used by the backend.
#[utoipa::path(
    responses(
        (status = 200, description = "Got rendered chat successfully.", body = RenderedChat),
        (status = 500, description = "Database operation failed.", body = String)
    )
)]
#[get("/<chat_id>/rendered")]
pub fn get_rendered_chat_by_id(
    db_conn: &State<Mutex<rusqlite::Connection>>,
    chat_id: Uuid,
) -> Result<Json<RenderedChat>, ApiErrorResponse> {
    let db_conn = db_conn.lock().unwrap();

    let chat = chats.get_chat_by_id(chat_id)?;
    let rendered_chat = RenderedChat::from_chat(&chat);

    Ok(Json(rendered_chat))
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PushMessageRequest<'a> {
    role: Role,
    text: &'a str,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PushMessageResponse;

/// Adds a message on to the end of a chat.
#[utoipa::path(
    responses(
        (status = 200, description = "Message added successfully.", body = PushMessageResponse),
        (status = 500, description = "Database operation failed.", body = String)
    )
)]
#[post("/<chat_id>/message", data = "<message>")]
pub fn push_message(
    db_conn: &State<Mutex<rusqlite::Connection>>,
    chat_id: Uuid,
    message: Json<PushMessageRequest>,
) -> Result<Json<PushMessageResponse>, ApiErrorResponse> {
    // TODO: Return 201 Created.

    let db_conn = db_conn.lock().unwrap();

    chat::push_message(&db_conn, chat_id, message.role, message.text)
        .map(|_| Json(PushMessageResponse {}))
        .map_err(From::from)
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct GenerateResponse {
    message: String,
}

/// Generates an assistant response on the end of a chat.
#[utoipa::path(
    responses(
        (status = 200, description = "Message generated succesfully.", body = GenerateResponse),
        (status = 500, description = "Database operation failed.", body = String),
        (status = 502, description = "LLM backend error.", body = String)
    ),
    params(
        ("continue_last_message", Query, description = "Whether to continue the last assistant message")
    )
)]
#[post("/<chat_id>/generate?<continue_last_message>")]
pub async fn generate(
    db_conn: &State<Mutex<rusqlite::Connection>>,
    anthropic_api: &State<AnthropicApi>,
    chat_id: Uuid,
    continue_last_message: bool,
) -> Result<Json<GenerateResponse>, ApiErrorResponse> {
    let chat = {
        let db_conn = db_conn.lock().unwrap();
        let chat = chat::get_chat_by_id(&db_conn, chat_id)?;
        RenderedChat::from_chat(&chat)
    };

    // TODO: Let client decide this, probably by choosing stored config values
    let anthropic_config = anthropic::Config::default();

    // TODO for continue: Anthropic API returned error 'invalid_request_error': messages: final assistant content cannot end with trailing whitespace
    let mut new_message = anthropic_api.generate(&anthropic_config, &chat).await?;

    {
        let db_conn = db_conn.lock().unwrap();

        if continue_last_message {
            // Some backends won't allow continuing a user message, but some will and we want to support it.
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
