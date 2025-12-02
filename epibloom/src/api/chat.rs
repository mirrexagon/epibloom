use std::sync::Mutex;

use rocket::{
    get,
    response::status,
    response::stream::{Event, EventStream},
    serde::json::Json,
    State,
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, OpenApi, ToSchema};
use uuid::Uuid;

use crate::{
    backends::{
        anthropic::{self, AnthropicApi},
        Backend,
    },
    data::chat::{self, Chat, ChatMetadata, ChatSession, ChatSessionEvent, ChatSessionStore, Role},
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
#[openapi(paths(get_chats, stream_chat_by_id))]
pub struct ChatApiDoc;

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

/// Stream the current state of the chat and any future message deltas.
#[utoipa::path(
    responses(
        (status = 200, description = "Got chat succesfully.", body = Chat),
        (status = 500, description = "Database operation failed.", body = String)
    )
)]
#[get("/<chat_id>")]
pub async fn stream_chat_by_id(
    db_conn: &State<Mutex<rusqlite::Connection>>,
    chat_id: Uuid,
    store: &State<ChatSessionStore>,
) -> Result<EventStream![], ApiErrorResponse> {
    let db_conn = db_conn.lock().unwrap();

    let session_arc =
        store
            .get_or_create_session(&db_conn, chat_id)
            .await
            .ok_or(ApiErrorResponse::NotFound(format!(
                "Chat with id {} not found",
                chat_id
            )))?;

    drop(db_conn);

    Ok(EventStream! {
        {
            let session = session_arc.read().await;

            // Subscribe to live updates before sending current chat state to avoid missing messages.
            let mut rx = session.get_new_receiver();

            for (position, message) in session.get_complete_messages().into_iter().enumerate() {
                match message.role {
                    Role::User => {
                        yield Event::json(&ChatSessionEvent::UserMessageStart { position });
                        yield Event::json(&ChatSessionEvent::UserMessageDelta { delta: message.text.clone() });
                        yield Event::json(&ChatSessionEvent::UserMessageEnd { position });
                    }

                    Role::Assistant => {
                        yield Event::json(&ChatSessionEvent::AssistantMessageStart { position });
                        if !message.thinking.is_empty() {
                            yield Event::json(&ChatSessionEvent::AssistantThinkingDelta { delta: message.thinking.clone() });
                        }
                        yield Event::json(&ChatSessionEvent::AssistantMessageDelta { delta: message.text.clone() });
                        yield Event::json(&ChatSessionEvent::AssistantMessageEnd { position });
                    }

                    Role::System => {}
                }
            }
        }
    })
}
