use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use time::macros::format_description;
use time::OffsetDateTime;
use uuid::Uuid;

// TODO: Assistant personas (characters) - chats live under assistant personas

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum Role {
    #[serde(rename = "assistant")]
    Assistant,
    #[serde(rename = "user")]
    User,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    role: Role,
    message: String,
}

impl Message {
    pub fn role(&self) -> Role {
        self.role
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chat {
    name: String,
    messages: Vec<Message>,
}

impl Chat {
    pub fn new(name: &str) -> Self {
        Chat {
            name: name.to_owned(),
            messages: Vec::new(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    /// Append a message to the end of the chat.
    ///
    /// Returns the index of the newly-pushed message.
    pub fn push_message(&mut self, role: Role, message: &str) -> usize {
        self.messages.push(Message {
            role,
            message: message.to_owned(),
        });

        self.messages.len() - 1
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chats {
    chats: HashMap<Uuid, Chat>,
}

impl Chats {
    pub fn new() -> Self {
        Chats {
            chats: HashMap::new(),
        }
    }

    pub fn chats(&self) -> &HashMap<Uuid, Chat> {
        &self.chats
    }

    pub fn chats_mut(&mut self) -> &mut HashMap<Uuid, Chat> {
        &mut self.chats
    }

    pub fn new_chat(&mut self, name: Option<&str>) -> Uuid {
        let new_chat_id = Uuid::new_v4();

        assert!(!self.chats.contains_key(&new_chat_id));

        let new_chat_name = if let Some(name) = name {
            name.to_owned()
        } else {
            let current_datetime = OffsetDateTime::now_local().unwrap();
            let dtfmt =
                format_description!(version = 2, "[year]-[month]-[day]T[hour][minute][second]");
            current_datetime.format(&dtfmt).unwrap()
        };

        self.chats.insert(new_chat_id, Chat::new(&new_chat_name));

        new_chat_id
    }
}
