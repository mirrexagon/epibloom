use std::collections::HashMap;

use uuid::Uuid;

use rocket::serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub enum Role {
    #[serde(rename = "assistant")]
    Assistant,
    #[serde(rename = "user")]
    User,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct Message {
    pub role: Role,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct Chat {
    pub system_prompt: String,
    pub messages: Vec<Message>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct Chats(pub HashMap<Uuid, Chat>);

impl Chats {
    pub fn new() -> Self {
        Chats(HashMap::new())
    }
}
