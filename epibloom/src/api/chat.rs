use std::sync::{Arc, Mutex};

use r2d2_sqlite::SqliteConnectionManager;
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
        featherless::{self, FeatherlessApi},
        Backend,
    },
    data::chat::{self, Chat, ChatMetadata, Role, Session, SessionEvent, SessionStore},
};

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
    db_pool: &State<r2d2::Pool<SqliteConnectionManager>>,
) -> Result<Json<GetChatsResponse>, ApiErrorResponse> {
    let db_conn = db_pool.get().unwrap();

    let chats_list = chat::db::list_chats(db_pool)?;

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
    db_pool: &State<r2d2::Pool<SqliteConnectionManager>>,
    chat_id: Uuid,
    store: &State<SessionStore>,
) -> Result<EventStream![], ApiErrorResponse> {
    let session_arc =
        store
            .get_or_create_session(db_pool, chat_id)
            .await
            .ok_or(ApiErrorResponse::NotFound(format!(
                "Chat with id {} not found",
                chat_id
            )))?;

    Ok(EventStream! {
        let (mut rx, past_messages, current_assistant_thinking, current_assistant_message, state) = {
            let session = session_arc.read().await;
            let (current_assistant_thinking, current_assistant_message) = session.get_in_progress_message();
            (session.get_new_receiver(), session.get_complete_messages(), current_assistant_thinking, current_assistant_message, session.get_state())
        };

        // Send past messages.
        for (position, message) in past_messages.iter().enumerate() {
            log::debug!("Sending past message {position}: {message:#?}");

            match message.role {
                Role::User => {
                    yield Event::json(&SessionEvent::UserMessageStart { position });
                    yield Event::json(&SessionEvent::UserMessageDelta { delta: message.text.clone() });
                    yield Event::json(&SessionEvent::UserMessageEnd { position });
                }

                Role::Assistant => {
                    yield Event::json(&SessionEvent::AssistantMessageStart { position });
                    if let Some(ref thinking) = message.thinking {
                        yield Event::json(&SessionEvent::AssistantThinkingDelta { delta: thinking.clone() });
                    }
                    yield Event::json(&SessionEvent::AssistantMessageDelta { delta: message.text.clone() });
                    yield Event::json(&SessionEvent::AssistantMessageEnd { position });
                }

                Role::System => {}
            }
        }

        // Send current message so far.
        if let chat::SessionState::Generating = state {
            yield Event::json(&SessionEvent::AssistantMessageStart { position: past_messages.len() });

            if !current_assistant_thinking.is_empty() {
                yield Event::json(&SessionEvent::AssistantThinkingDelta { delta: current_assistant_thinking });
            }

            if !current_assistant_message.is_empty() {
                yield Event::json(&SessionEvent::AssistantMessageDelta { delta: current_assistant_message });
            }
        }

        // TODO: Is there ever a case where the above events occur and the
        // message is also complete and we would have to send a message end
        // event here? Ideally this should be impossible, either the message is
        // in progress and the complete will come later, or it is not generating
        // and the most recent message is part of the past messages.

        // Send any new events/messages as they come in.
        use tokio::sync::broadcast::error::RecvError;

        loop {
            match rx.recv().await {
                Ok(event) => {
                    // TODO: Sometimes getting just a `:` on the stream as a client once the stream is over?
                    // Seems to be probably a heartbeat, eg. https://stackoverflow.com/questions/76004062/how-to-send-a-ping-event-in-sse-spring-web
                    log::debug!("Channel received event: {event:#?}");
                    yield Event::json(&event);
                }
                Err(RecvError::Lagged(num_skipped_messages)) => {
                    log::error!("Lagged behind {} messages from chat stream", num_skipped_messages);
                }
                Err(RecvError::Closed) => break,
            }
        }
    })
}

#[post("/testgenerate")]
pub async fn test_generate(
    db_pool: &State<r2d2::Pool<SqliteConnectionManager>>,
    store: &State<SessionStore>,
    featherless_api: &State<FeatherlessApi>,
) -> Result<Json<ChatMetadata>, ApiErrorResponse> {
    let db_conn = db_pool
        .get()
        .map_err(|err| ApiErrorResponse::InternalServerError(err.to_string()))?;

    let new_chat_metadata = chat::new_chat(db_pool, None)
        .map_err(|err| ApiErrorResponse::InternalServerError(err.to_string()))?;

    log::info!("Created new chat: {:#?}", new_chat_metadata);

    // TODO: Do this through the session to keep the session in sync, or have
    // start_generating pull the latest chat from database - maybe even it won't
    // store a mirror of the chat from the database and always query it (or
    // maybe just when not generating).
    chat::push_message(
        db_pool,
        new_chat_metadata.id,
        Role::User,
        None,
        "What are the primary nutrients in orange juice?",
    )
    .map_err(|err| ApiErrorResponse::InternalServerError(err.to_string()))?;

    let session = store
        .get_or_create_session(db_pool, new_chat_metadata.id)
        .await
        .unwrap();

    // TODO: Have start_generating take a chat ID, not a session. In general,
    // sessions should be an implementation detail and there should be methods
    // on the store to get and do these things by chat ID.
    let backend = Arc::new((*featherless_api).clone());
    let config = Arc::new(featherless::Config::default());

    chat::Session::start_generating(session, backend, config, db_pool)
        .await
        .map_err(|err| ApiErrorResponse::InternalServerError(err.to_string()))?;

    Ok(Json(new_chat_metadata))
}
