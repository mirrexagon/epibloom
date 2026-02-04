use std::collections::HashMap;
use std::sync::Arc;

use async_stream::stream;
use futures::Stream;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, Mutex, RwLock};
use typemap_rev::TypeMapKey;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::backend::{Backend, Backends, GenerateChunk};

use super::{Chat, ChatMetadata, Error, Message, Role};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatStoreEvent {
    Added { chat: ChatMetadata },
}

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
    sessions: RwLock<HashMap<Uuid, Arc<RwLock<Session>>>>,
    backends: Mutex<Backends>,
    event_broadcast: broadcast::Sender<ChatStoreEvent>,
}

impl ChatStore {
    pub fn new() -> Self {
        let (event_broadcast, _) = broadcast::channel(32);

        Self {
            sessions: RwLock::new(HashMap::new()),
            backends: Mutex::new(Backends::new()),
            event_broadcast,
        }
    }

    pub async fn load_chats_from_chats_directory(&self) -> Result<(), Error> {
        let chats = Chat::load_all_from_chats_directory().await?;

        for chat in chats {
            self.add_chat(chat).await?;
        }

        Ok(())
    }

    pub async fn save_chat(&self, chat_id: Uuid) -> Result<(), Error> {
        let sessions = self.sessions.read().await;
        let session_arc = sessions.get(&chat_id).ok_or(Error::ChatNotFound(chat_id))?;
        let session = session_arc.read().await;
        session.chat.save_to_chats_directory().await?;
        Ok(())
    }

    pub async fn save_all_chats(&self) -> Result<(), Error> {
        let sessions = self.sessions.read().await;

        for (_, session_arc) in sessions.iter() {
            let session = session_arc.read().await;
            session.chat.save_to_chats_directory().await?;
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
        let chat = Chat::new(name);
        let new_chat_id = chat.metadata.id;

        log::info!("Creating chat: {new_chat_id}: '{}'", chat.metadata.name);

        chat.save_to_chats_directory().await?;
        self.add_chat(chat).await?;
        Ok(new_chat_id)
    }

    async fn add_chat(&self, chat: Chat) -> Result<(), Error> {
        let (broadcast, _) = broadcast::channel(1024);
        let new_chat_metadata = chat.metadata.clone();
        let new_chat_id = new_chat_metadata.id;
        let new_session = Session {
            chat,
            current_assistant_message: String::new(),
            current_assistant_thinking: String::new(),
            broadcast,
            state: State::Idle,
        };

        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(new_chat_id, Arc::new(RwLock::new(new_session)));

            // We don't care if there are no receivers.
            let _ = self.event_broadcast.send(ChatStoreEvent::Added {
                chat: new_chat_metadata,
            });
        }

        Ok(())
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

    // TODO: Maybe just set the entire chat instead of having functions to push messages and stuff.
    // The Lua API can update last modified times etc.
    // The Lua LuaChat object can have messages indexed by integers directly in
    // the chat object. Only problem is doing things
    // like `chat[1].text = "text"` directly.
    // Also should probably be more like parent object is a chat store chat
    // reference and the individual chat object is just a chat, since it has
    // metadata like ID and times. Or you get and set these chat copy objects on the
    // chat store directly.

    pub async fn push_message(
        &self,
        chat_id: Uuid,
        role: Role,
        text: &str,
        thinking: Option<&str>,
    ) -> Result<usize, Error> {
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
        session.chat.save_to_chats_directory().await?;

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

        Ok(new_message_position)
    }

    pub async fn get_chat_store_event_stream(
        &self,
    ) -> Result<impl Stream<Item = ChatStoreEvent>, Error> {
        // Get the current existing sessions. We also want to lock sessions so
        // that no one can modify it while we subscribe to adds/removes and get
        // the current chat list.
        let sessions = self.sessions.read().await;

        let mut initial_chat_store_events = Vec::new();
        for (_chat_id, session_arc) in sessions.iter() {
            let session = session_arc.read().await;
            initial_chat_store_events.push(ChatStoreEvent::Added {
                chat: session.chat.metadata.clone(),
            });
        }

        let mut rx = self.event_broadcast.subscribe();

        // TODO: This is inefficient since it has to lock every chat. Is there a
        // better way to store chats so the metadata is more easily accessible?

        Ok(stream! {
            // Send current chat list.
            for event in initial_chat_store_events {
                yield event;
            }


            // Send any changes to the chat list.
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

    pub async fn start_generating<B: Backend + TypeMapKey<Value = Arc<B>> + Send + Sync>(
        &self,
        chat_id: Uuid,
        backend_config: B::Config,
    ) -> Result<(), Error>
    where
        <B as Backend>::Config: Send + Sync + 'static,
        <B as Backend>::Error: Send + Sync + 'static,
    {
        let session_arc = self.get_session_arc(chat_id).await?;
        let backend = self
            .backends
            .lock()
            .await
            .get::<B>()
            .map_err(|err| Error::Backend(err.to_string()))?;
        Session::start_generating::<B>(session_arc, backend, backend_config).await?;
        Ok(())
    }

    // TODO: Cancel a chat that is generating
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Idle,
    Generating,
}

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

    async fn start_generating<B: Backend>(
        session_arc: Arc<RwLock<Session>>,
        backend: Arc<B>,
        backend_config: B::Config,
    ) -> Result<(), Error>
    where
        <B as Backend>::Config: Send + Sync + 'static,
        <B as Backend>::Error: Send + Sync + 'static,
    {
        let chat = {
            let mut session = session_arc.write().await;
            if let State::Generating = session.state {
                return Err(Error::ChatAlreadyGenerating);
            }

            session.state = State::Generating;
            session.chat.clone()
        };

        let stream = backend.generate(&backend_config, &chat);

        tokio::spawn(async move {
            let _ =
                process_generation_stream(session_arc.clone(), stream, chat.messages.len()).await;
            // If an error occurs in process_generation_stream(), a
            // ChatEvent::Error will be sent on the session's broadcast channel.

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

                if let Err(err) = session.chat.save_to_chats_directory().await {
                    let _ = session.broadcast.send(ChatEvent::Error {
                        message: err.to_string(),
                    });
                }
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
    S: futures::Stream<Item = Result<GenerateChunk, E>>,
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
                GenerateChunk::ThinkingDelta(ref delta) => {
                    session.current_assistant_thinking.push_str(delta);

                    // Ignore failure to send as there may be no receivers.
                    let _ = session.broadcast.send(ChatEvent::AssistantThinkingDelta {
                        delta: delta.clone(),
                    });
                }
                GenerateChunk::MessageDelta(ref delta) => {
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
