pub mod chat;

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
