use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_stream::stream;
use futures::Stream;
use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::sync::{broadcast, RwLock};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::backends;

use super::{Chat, ChatMetadata, Error, Message, Role};

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
    chat_directory: PathBuf,
    sessions: Arc<RwLock<HashMap<Uuid, Arc<RwLock<Session>>>>>,
}

impl ChatStore {
    pub fn new<P: AsRef<Path>>(chat_directory: P) -> Self {
        Self {
            chat_directory: chat_directory.as_ref().to_owned(),
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn save_chat(&self, chat_id: Uuid) -> Result<(), Error> {
        let mut file_path = self.chat_directory.clone();
        let sessions = self.sessions.read().await;

        let session_arc = sessions.get(&chat_id).ok_or(Error::ChatNotFound(chat_id))?;
        let session = session_arc.read().await;

        file_path.push(format!("{}.json", session.chat.metadata.id.hyphenated()));
        fs::write(&file_path, serde_json::to_string_pretty(&session.chat)?).await?;

        Ok(())
    }

    pub async fn save_all_chats(&self) -> Result<(), Error> {
        let mut file_path = self.chat_directory.clone();
        let sessions = self.sessions.read().await;

        for (_, session_arc) in sessions.iter() {
            let session = session_arc.read().await;
            file_path.push(format!("{}.json", session.chat.metadata.id.hyphenated()));
            fs::write(&file_path, serde_json::to_string_pretty(&session.chat)?).await?;
            file_path.pop();
        }

        Ok(())
    }

    pub async fn list_chats(&self) -> Result<Vec<ChatMetadata>, super::Error> {
        let sessions = self.sessions.read().await;

        // TODO: This is inefficient since it has to lock every chat. Is there a
        // better way to store chats so the metadata is more easily accessible?
        let mut chats = Vec::new();
        for (_, session_arc) in sessions.iter() {
            let session = session_arc.read().await;
            chats.push(session.chat.metadata.clone());
        }

        Ok(chats)
    }

    pub async fn new_chat(&self, name: Option<&str>) -> Result<Uuid, Error> {
        let new_chat_id = {
            let (broadcast, _) = broadcast::channel(1024);
            let chat = Chat::new(name);
            let new_chat_id = chat.metadata.id;
            let new_session = Session {
                chat,
                current_assistant_message: String::new(),
                current_assistant_thinking: String::new(),
                broadcast,
                state: State::Idle,
            };

            let mut sessions = self.sessions.write().await;

            sessions.insert(new_chat_id, Arc::new(RwLock::new(new_session)));

            new_chat_id
        };

        self.save_chat(new_chat_id).await?;

        Ok(new_chat_id)
    }

    async fn get_session_arc(&self, chat_id: Uuid) -> Result<Arc<RwLock<Session>>, Error> {
        let sessions = self.sessions.read().await;

        match sessions.get(&chat_id) {
            Some(session_arc) => Ok(session_arc.clone()),
            None => Err(Error::ChatNotFound(chat_id)),
        }
    }

    pub async fn get_chat_copy(&self, chat_id: Uuid) -> Result<Chat, Error> {
        let session_arc = self.get_session_arc(chat_id).await?;
        let session = session_arc.read().await;
        Ok(session.chat.clone())
    }

    pub async fn push_message(
        &self,
        chat_id: Uuid,
        role: Role,
        thinking: Option<&str>,
        text: &str,
    ) -> Result<usize, Error> {
        let new_message_position = {
            if thinking.is_some() && role != Role::Assistant {
                return Err(Error::ThinkingSuppliedForNonAssistantMessage);
            }

            let session_arc = self.get_session_arc(chat_id).await?;
            let mut session = session_arc.write().await;

            if session.get_state() == State::Generating {
                return Err(Error::ChatAlreadyGenerating);
            }

            let now = time::OffsetDateTime::now_local().unwrap();

            session.chat.messages.push(Message {
                role,
                thinking: thinking.map(|s| s.to_owned()),
                text: text.to_owned(),
                created: now,
                modified: now,
            });

            session.chat.metadata.modified = now;

            let new_message_position = session.chat.messages.len() - 1;

            let _ = session.broadcast.send(match role {
                Role::System | Role::User => ChatEvent::UserMessageStart {
                    position: new_message_position,
                },
                Role::Assistant => ChatEvent::AssistantMessageStart {
                    position: new_message_position,
                },
            });

            if let Some(thinking) = thinking {
                let _ = session.broadcast.send(ChatEvent::AssistantThinkingDelta {
                    delta: thinking.to_owned(),
                });
            }

            let _ = session.broadcast.send(match role {
                Role::System | Role::User => ChatEvent::UserMessageDelta {
                    delta: text.to_owned(),
                },
                Role::Assistant => ChatEvent::AssistantMessageDelta {
                    delta: text.to_owned(),
                },
            });

            let _ = session.broadcast.send(match role {
                Role::System | Role::User => ChatEvent::UserMessageStart {
                    position: new_message_position,
                },
                Role::Assistant => ChatEvent::AssistantMessageStart {
                    position: new_message_position,
                },
            });

            new_message_position
        };

        self.save_chat(chat_id).await?;

        Ok(new_message_position)
    }

    /// Get a stream that yields events corresponding to existing messages, then any future message chunks.
    pub async fn get_chat_stream(
        &self,
        chat_id: Uuid,
    ) -> Result<impl Stream<Item = ChatEvent>, Error> {
        let session_arc = self.get_session_arc(chat_id).await?;

        Ok(stream! {
            let (mut rx, past_messages, current_assistant_thinking, current_assistant_message, state) = {
                let session = session_arc.read().await;
                let (current_assistant_thinking, current_assistant_message) =
                    session.get_in_progress_message();
                (
                    session.get_new_receiver(),
                    session.get_complete_messages(),
                    current_assistant_thinking,
                    current_assistant_message,
                    session.get_state(),
                )
            };

            // Send past messages.
            for (position, message) in past_messages.iter().enumerate() {
                match message.role {
                    Role::User => {
                        yield ChatEvent::UserMessageStart { position };
                        yield ChatEvent::UserMessageDelta {
                            delta: message.text.clone(),
                        };
                        yield ChatEvent::UserMessageEnd { position };
                    }

                    Role::Assistant => {
                        yield ChatEvent::AssistantMessageStart { position };
                        if let Some(ref thinking) = message.thinking {
                            yield ChatEvent::AssistantThinkingDelta {
                                delta: thinking.clone(),
                            };
                        }
                        yield ChatEvent::AssistantMessageDelta {
                            delta: message.text.clone(),
                        };
                        yield ChatEvent::AssistantMessageEnd { position };
                    }

                    Role::System => {}
                }
            }

            // Send current message so far.
            if let State::Generating = state {
                yield ChatEvent::AssistantMessageStart {
                    position: past_messages.len(),
                };

                if !current_assistant_thinking.is_empty() {
                    yield ChatEvent::AssistantThinkingDelta {
                        delta: current_assistant_thinking,
                    };
                }

                if !current_assistant_message.is_empty() {
                    yield ChatEvent::AssistantMessageDelta {
                        delta: current_assistant_message,
                    };
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
                        yield event;
                    }
                    Err(RecvError::Lagged(num_skipped_messages)) => {
                        log::error!(
                            "Lagged behind {} messages from chat stream",
                            num_skipped_messages
                        );
                    }
                    Err(RecvError::Closed) => break,
                }
            }
        })
    }

    pub async fn start_generating<B: backends::Backend>(
        &self,
        chat_id: Uuid,
        backend: &B,
        backend_config: &B::Config,
    ) -> Result<(), Error>
    where
        <B as backends::Backend>::Config: Send + Sync + 'static,
        <B as backends::Backend>::Error: Send + Sync + 'static,
    {
        let session_arc = self.get_session_arc(chat_id).await?;
        Session::start_generating(session_arc, backend, backend_config).await?;
        Ok(())
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

    async fn start_generating<B: backends::Backend>(
        session_arc: Arc<RwLock<Session>>,
        backend: &B,
        backend_config: &B::Config,
    ) -> Result<(), Error>
    where
        <B as backends::Backend>::Config: Send + Sync + 'static,
        <B as backends::Backend>::Error: Send + Sync + 'static,
    {
        let chat = {
            let mut session = session_arc.write().await;
            if let State::Generating = session.state {
                return Err(Error::ChatAlreadyGenerating);
            }

            session.state = State::Generating;
            session.chat.clone()
        };

        let stream = backend.generate(backend_config, &chat);

        tokio::spawn(async move {
            process_generation_stream(session_arc.clone(), stream, chat.messages.len())
                .await
                .unwrap();

            // Stream has completed.
            {
                let mut session = session_arc.write().await;

                let new_thinking = session.current_assistant_thinking.clone();
                let new_message = session.current_assistant_message.clone();

                let now = time::OffsetDateTime::now_local().unwrap();

                session.chat.messages.push(Message {
                    role: Role::Assistant,
                    thinking: if !new_thinking.is_empty() {
                        Some(new_thinking)
                    } else {
                        None
                    },
                    text: new_message,
                    created: now,
                    modified: now,
                });

                session.current_assistant_message.truncate(0);
                session.current_assistant_thinking.truncate(0);

                session.chat.metadata.modified = now;

                session.state = State::Idle;
            }

            // TODO: Save chat here.
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
