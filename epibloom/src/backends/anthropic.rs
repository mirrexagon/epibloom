use std::env;

use log::info;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sse_stream::SseStream;
use time::OffsetDateTime;

use crate::data::chat;

// https://docs.anthropic.com/en/api/versioning
const ANTHROPIC_API_VERSION: &str = "2023-06-01";
const ANTHROPIC_API_URL_BASE: &str = "https://api.anthropic.com/v1";

pub struct AnthropicApi {
    api_key: String,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),
}

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    r#type: String,
    id: String,
    display_name: String,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModelInfoPage {
    data: Vec<ModelInfo>,
    has_more: bool,
    first_id: String,
    last_id: String,
}

// /messages
#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum Role {
    #[serde(rename = "assistant")]
    Assistant,
    #[serde(rename = "user")]
    User,
}

impl From<chat::Role> for Role {
    fn from(value: chat::Role) -> Self {
        match value {
            chat::Role::Assistant => Role::Assistant,
            chat::Role::User => Role::User,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Message {
    role: Role,
    content: String,
}

impl From<chat::Message> for Message {
    fn from(message: chat::Message) -> Self {
        Message {
            role: From::from(message.role),
            content: message.text,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MessagesRequest {
    max_tokens: u32,
    messages: Vec<Message>,
    model: String,
    stop_sequences: Vec<String>,
    stream: bool,
    system: String,
}

impl Default for MessagesRequest {
    fn default() -> Self {
        MessagesRequest {
            max_tokens: 256,
            messages: Vec::new(),
            model: "claude-3-haiku-20240307".to_string(),
            stop_sequences: Vec::new(),
            stream: false,
            system: String::new(),
        }
    }
}

impl From<chat::Chat> for MessagesRequest {
    fn from(chat: chat::Chat) -> Self {
        let mut request = MessagesRequest::default();

        request.messages = chat
            .messages
            .iter()
            .map(Clone::clone)
            .map(From::from)
            .collect();

        request
    }
}

impl AnthropicApi {
    pub fn new_with_key_from_env() -> Self {
        let api_key = env::var("ANTHROPIC_API_KEY").unwrap();

        AnthropicApi { api_key }
    }

    pub async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        let client = reqwest::Client::new();
        let url = format!("{base}/models", base = ANTHROPIC_API_URL_BASE);

        let response = client
            .get(url)
            .header("x-api-key", self.api_key.clone())
            .header("anthropic-version", ANTHROPIC_API_VERSION)
            .header("content-type", "application/json")
            .send()
            .await?;

        let body: ModelInfoPage = response.json().await?;
        info!("{:?}", body);

        // TODO: Concatenate all pages in the model list.
        Ok(body.data)
    }

    // TODO: Expose streaming outside of this
    pub async fn generate(&self, chat: chat::Chat) -> Result<String> {
        let client = reqwest::Client::new();
        let url = format!("{base}/messages", base = ANTHROPIC_API_URL_BASE);

        let body = MessagesRequest::from(chat);

        let response = client
            .post(url)
            .header("x-api-key", self.api_key.clone())
            .header("anthropic-version", ANTHROPIC_API_VERSION)
            .header("content-type", "application/json")
            .send()
            .await?;

        let mut sse_body = SseStream::from_byte_stream(response.bytes_stream());
        let mut generated_message = String::new();

        while let Some(Ok(sse)) = sse_body.next().await {
            // TODO
        }

        Ok(generated_message)
    }
}

pub mod api {
    use rocket::{get, http::Status, response::status, serde::json::Json, State};

    use super::*;

    /*
    #[get("/models")]
    pub async fn list_models(
        anthropic_api: &State<AnthropicApi>,
    ) -> Result<Json<Vec<ModelInfo>>, status::Custom<String>> {
        let model_list = anthropic_api
            .list_models()
            .await
            .map_err(|err| {
                status::Custom(
                    Status::BadGateway,
                    format!("Failed to get model list from Anthropic: {err}"),
                )
            })
            .map_err(|err| err.to_string());

        Ok(Json(model_list))
    }
    */
}
