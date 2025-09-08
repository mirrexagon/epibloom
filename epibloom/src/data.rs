pub mod chat;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RenderedChat {
    pub messages: Vec<chat::Message>,
}

impl RenderedChat {
    pub fn from_chat(chat: &chat::Chat) -> Self {
        Self {
            messages: chat.messages.clone(),
        }
    }
}
