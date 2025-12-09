use std::{collections::HashMap, fmt, sync::Arc};

use async_stream::stream;
use futures::{Stream, StreamExt};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use time::macros::format_description;
use time::OffsetDateTime;
use tokio::sync::{broadcast, RwLock};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::backends;
use crate::data;
use crate::util;

use super::{db, Chat, ChatMetadata, Error, Message};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatEvent {
    UserMessageStart { position: usize },
    UserMessageDelta { delta: String },
    UserMessageEnd { position: usize },
    AssistantMessageStart { position: usize },
    AssistantThinkingDelta { delta: String },
    AssistantMessageDelta { delta: String },
    AssistantMessageEnd { position: usize },
    Error { message: String },
}

pub struct ChatStore {
    sessions: Arc<RwLock<HashMap<Uuid, Arc<RwLock<Session>>>>>,
}

impl ChatStore {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn list_chats(
        db_pool: &r2d2::Pool<SqliteConnectionManager>,
    ) -> Result<Vec<ChatMetadata>, rusqlite::Error> {
        db::list_chats(db_pool)
    }

    pub async fn push_message(chat_id: Uuid) -> Result<usize, Error> {
        // TODO: Make sure there is no active session, then push message into database

        unimplemented!()
    }

    pub async fn get_chat_stream(
        &self,
        db_pool: &r2d2::Pool<SqliteConnectionManager>,
        chat_id: Uuid,
    ) -> Result<impl Stream<Item = ChatEvent>, Error> {
        // TODO: Return a stream that yields all messages up to this point, any in-progress message, and any future events.
        // The session will have to live until all such streams are dropped and
        // no generation is happening, so it somehow needs to know when all its
        // streams are dropped.

        let session_arc = match self.get_or_create_session(db_pool, chat_id).await {
            Some(session_arc) => Ok(session_arc),
            None => Err(Error::ChatNotFound(chat_id)),
        }?;

        Ok(stream! {
            // TODO: Implement the stream
            yield ChatEvent::UserMessageStart { position: 0 };
        })
    }

    // TODO: Removing sessions when no clients are active and no generation is happening.

    /// Returns `None` if there is no chat with the given ID.
    async fn get_or_create_session(
        &self,
        db_pool: &r2d2::Pool<SqliteConnectionManager>,
        chat_id: Uuid,
    ) -> Option<Arc<RwLock<Session>>> {
        match db::get_chat_by_id(db_pool, chat_id) {
            Ok(chat) => {
                let (broadcast, _) = broadcast::channel(1024);
                let mut sessions = self.sessions.write().await;

                Some(
                    sessions
                        .entry(chat_id)
                        .or_insert_with(|| {
                            Arc::new(RwLock::new(Session {
                                chat,
                                current_assistant_message: String::new(),
                                current_assistant_thinking: String::new(),
                                broadcast,
                                state: State::Idle,
                            }))
                        })
                        .clone(),
                )
            }

            Err(err) => {
                log::error!("Failed to get or create chat session {chat_id:}: {err}");
                None
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Idle,
    Generating,
}

/// Live chat streaming sessions created on demand when either starting
/// generation or requesting a session. These are used mainly to be the single
/// source of streaming generation for the chat.
struct Session {
    chat: Chat,

    current_assistant_thinking: String,
    /// Current incomplete assistant message. Will be added to the chat once it is complete.
    current_assistant_message: String,

    broadcast: broadcast::Sender<ChatEvent>,
    state: State,
}

#[derive(Debug, thiserror::Error)]
pub enum StartGeneratingError {
    #[error("chat is already generating")]
    ChatAlreadyGenerating,
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
}

impl Session {
    fn get_new_receiver(&self) -> broadcast::Receiver<ChatEvent> {
        self.broadcast.subscribe()
    }

    fn get_complete_messages(&self) -> Vec<Message> {
        self.chat.messages.clone()
    }

    fn get_in_progress_message(&self) -> (String, String) {
        (
            self.current_assistant_thinking.clone(),
            self.current_assistant_message.clone(),
        )
    }

    fn get_state(&self) -> State {
        self.state
    }

    // TODO: This needs to tak Arcs for backend and backend_config because
    // Backend::generate() takes references to the chat and config and the
    // stream has those lifetimes tied to it. Break the lifetime links between
    // argument and returned stream somehow, so the ongoing stream doesn't
    // depend on the exact inputs.
    async fn start_generating<B: backends::Backend + Send + Sync + 'static>(
        session_arc: Arc<RwLock<Session>>,
        backend: Arc<B>,
        backend_config: Arc<B::Config>,
        db_pool: &r2d2::Pool<SqliteConnectionManager>,
    ) -> Result<(), StartGeneratingError>
    where
        <B as backends::Backend>::Error: Send + Sync + 'static,
        <B as backends::Backend>::Config: Send + Sync + 'static,
    {
        let chat = {
            let mut session = session_arc.write().await;
            if let State::Generating = session.state {
                return Err(StartGeneratingError::ChatAlreadyGenerating);
            }

            session.state = State::Generating;
            session.chat.clone()
        };

        tokio::spawn(async move {
            let stream = backend.generate(&backend_config, &chat);
            process_generation_stream(session_arc.clone(), stream, chat.messages.len())
                .await
                .unwrap();

            // Stream has completed.
            {
                let mut session = session_arc.write().await;

                // TODO: Push message to database
                // TODO: Push thinking
                /*
                    data::chat::push_message(
                        db_conn,
                        session.chat.metadata.id,
                        Role::Assistant,
                        &session.current_assistant_message,
                    )
                    .unwrap();
                */

                let new_thinking = session.current_assistant_thinking.clone();
                let new_message = session.current_assistant_message.clone();

                session.chat.messages.push(Message {
                    role: Role::Assistant,
                    thinking: if !new_thinking.is_empty() {
                        Some(new_thinking)
                    } else {
                        None
                    },
                    text: new_message,
                    created: time::UtcDateTime::now().into(),
                    modified: time::UtcDateTime::now().into(),
                });

                session.current_assistant_message.truncate(0);
                session.current_assistant_thinking.truncate(0);

                session.state = State::Idle;
            }
        });

        Ok(())
    }
}

async fn process_generation_stream<S, E>(
    session_arc: Arc<RwLock<Session>>,
    stream: S,
    message_position: usize,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    S: futures::Stream<Item = Result<backends::GenerateChunk, E>>,
    E: std::error::Error + Send + Sync + 'static,
{
    use futures::stream::StreamExt;

    futures::pin_mut!(stream);

    {
        let session = session_arc.write().await;
        let _ = session.broadcast.send(ChatEvent::AssistantMessageStart {
            position: message_position,
        });
    }

    while let Some(result) = stream.next().await {
        let mut session = session_arc.write().await;
        match result {
            Ok(chunk) => match chunk {
                backends::GenerateChunk::ThinkingDelta(ref delta) => {
                    session.current_assistant_thinking.push_str(delta);

                    // Ignore failure to send as there may be no receivers.
                    let _ = session.broadcast.send(ChatEvent::AssistantThinkingDelta {
                        delta: delta.clone(),
                    });
                }
                backends::GenerateChunk::MessageDelta(ref delta) => {
                    session.current_assistant_message.push_str(delta);
                    let _ = session.broadcast.send(ChatEvent::AssistantMessageDelta {
                        delta: delta.clone(),
                    });
                }
            },
            Err(err) => {
                let _ = session.broadcast.send(ChatEvent::Error {
                    message: err.to_string(),
                });
                return Err(Box::new(err));
            }
        }
    }

    {
        let session = session_arc.write().await;
        let _ = session.broadcast.send(ChatEvent::AssistantMessageEnd {
            position: message_position,
        });
    }

    Ok(())
}
