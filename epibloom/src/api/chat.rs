use std::sync::Arc;

use futures::StreamExt;
use rocket::{
    get,
    response::stream::{Event, EventStream},
    State,
};
use utoipa::OpenApi;
use uuid::Uuid;

use crate::chat::{self, ChatEvent, ChatStore, ChatStoreEvent};

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
#[openapi(paths(stream_chat_store_events, stream_chat_by_id))]
pub struct ChatApiDoc;

/// Stream all current debug chats as additions, then stream any subsequent
/// chats being added or removed.
///
/// Just includes metadata, no messages.
#[utoipa::path(
    responses(
        (status = 200, description = "An SSE stream of chat additions or removals.", body = ChatStoreEvent),
        (status = 500, description = "An error occurred starting the stream.", body = String)
    )
)]
#[get("/")]
pub async fn stream_chat_store_events(
    store: &State<Arc<ChatStore>>,
) -> Result<EventStream![], ApiErrorResponse> {
    let chat_store_stream = store.get_chat_store_event_stream().await.map_err(|err| {
        ApiErrorResponse::InternalServerError(format!(
            "Could not get chat store event stream: {}",
            err
        ))
    })?;

    Ok(EventStream! {
        futures::pin_mut!(chat_store_stream);

        while let Some(chat_store_event) = chat_store_stream.next().await {
            yield Event::json(&chat_store_event);
        }
    })
}

/// Stream the current state of the chat and any future message deltas.
#[utoipa::path(
    responses(
        (status = 200, description = "An SSE stream of message events.", body = ChatEvent),
        (status = 500, description = "An error occurred starting the stream.", body = String)
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
