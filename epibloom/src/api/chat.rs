use std::sync::Arc;

use futures::StreamExt;
use rocket::{
    get,
    response::stream::{Event, EventStream},
    serde::json::Json,
    State,
};
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};
use uuid::Uuid;

use crate::backend::{
    anthropic::{self, AnthropicApi},
    featherless::{self, FeatherlessApi},
};
use crate::chat::{self, Chat, ChatMetadata, ChatStore, Role};

use super::ApiErrorResponse;

impl From<chat::Error> for ApiErrorResponse {
    fn from(value: chat::Error) -> Self {
        match value {
            chat::Error::ChatNotFound(_)
            | chat::Error::ChatHasNoMessages(_)
            | chat::Error::MessageNotFound { .. } => ApiErrorResponse::NotFound(value.to_string()),
            chat::Error::ChatAlreadyGenerating => ApiErrorResponse::Conflict(value.to_string()),
            chat::Error::ThinkingSuppliedForNonAssistantMessage => {
                ApiErrorResponse::BadRequest(value.to_string())
            }
            chat::Error::Io(_) | chat::Error::Json(_) => {
                ApiErrorResponse::InternalServerError(value.to_string())
            }
            chat::Error::Backend(err) => ApiErrorResponse::BadGateway(err),
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
pub async fn get_chats(
    store: &State<Arc<ChatStore>>,
) -> Result<Json<GetChatsResponse>, ApiErrorResponse> {
    let chats_list = store.list_chats().await?;

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
    chat_id: Uuid,
    store: &State<Arc<ChatStore>>,
) -> Result<EventStream![], ApiErrorResponse> {
    let chat_stream = store.get_chat_stream(chat_id).await.map_err(|err| {
        ApiErrorResponse::NotFound(format!(
            "Could not get stream for chat {}: {}",
            chat_id, err
        ))
    })?;

    Ok(EventStream! {
        futures::pin_mut!(chat_stream);

        while let Some(chat_event) = chat_stream.next().await {
            yield Event::json(&chat_event);
        }
    })
}
