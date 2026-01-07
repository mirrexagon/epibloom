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

use crate::backends::{
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
            chat::Error::Io(_) => ApiErrorResponse::InternalServerError(value.to_string()),
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
    store: &State<ChatStore>,
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
    store: &State<ChatStore>,
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

#[post("/testgenerate")]
pub async fn test_generate(
    store: &State<ChatStore>,
    featherless_api: &State<FeatherlessApi>,
    anthropic_api: &State<AnthropicApi>,
) -> Result<Json<ChatMetadata>, ApiErrorResponse> {
    let new_chat_id = store.new_chat(None).await?;
    let new_chat_metadata = store.get_chat_copy(new_chat_id).await?.metadata;
    log::info!("Created new chat: {:#?}", new_chat_metadata);

    store
        .push_message(
            new_chat_id,
            Role::User,
            None,
            "What are the primary nutrients in orange juice?",
        )
        .await?;

    let config = anthropic::Config::default();
    store
        .start_generating(new_chat_id, &**anthropic_api, &config)
        .await?;

    Ok(Json(new_chat_metadata))
}
