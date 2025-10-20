//! Types and functions for the OpenAI-compatible chat completions endpoint.

use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use super::FEATHERLESS_API_URL_BASE;

use crate::backends;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Role {
    System,
    Assistant,
    User,
}

impl From<backends::Role> for Role {
    fn from(value: backends::Role) -> Self {
        match value {
            backends::Role::System => Role::System,
            backends::Role::Assistant => Role::Assistant,
            backends::Role::User => Role::User,
        }
    }
}

mod request {
    use serde::Serialize;

    use crate::backends;

    #[derive(Debug, Clone, Serialize)]
    pub struct Request {
        pub model: String,
        pub messages: Vec<Message>,
        pub max_tokens: u32,

        #[serde(skip_serializing_if = "Option::is_none")]
        pub temperature: Option<f32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub top_p: Option<f32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub top_k: Option<i32>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        pub stop: Vec<String>,
    }

    impl Request {
        pub fn new(config: &super::super::Config, chat: &backends::Chat) -> Self {
            Self {
                model: config.model.clone(),
                messages: chat.messages.iter().map(From::from).collect(),
                max_tokens: config.max_tokens,
                temperature: config.temperature,
                top_p: config.top_p,
                top_k: config.top_k,
                stop: config.stop.clone(),
            }
        }
    }

    #[derive(Debug, Clone, Serialize)]
    pub struct Message {
        pub role: super::Role,
        pub content: String,
    }

    impl From<&backends::Message> for Message {
        fn from(message: &backends::Message) -> Self {
            Message {
                role: From::from(message.role),
                content: message.text.clone(),
            }
        }
    }
}

mod response {
    use serde::Deserialize;

    #[derive(Debug, Clone, Deserialize)]
    pub struct ChatCompletion {
        pub id: String,
        pub object: String,
        pub created: u64,
        pub model: String,
        pub choices: Vec<Choice>,
        pub usage: Usage,
    }

    #[derive(Debug, Clone, Deserialize)]
    pub struct Choice {
        pub index: u32,
        pub message: Message,
        pub finish_reason: String,
    }

    #[derive(Debug, Clone, Deserialize)]
    pub struct Message {
        pub role: String,
        pub content: String,
    }

    #[derive(Debug, Clone, Deserialize)]
    pub struct Usage {
        pub prompt_tokens: u32,
        pub completion_tokens: u32,
        pub total_tokens: u32,
    }

    #[derive(Debug, Clone, Deserialize)]
    pub struct ErrorResponse {
        pub error: ErrorDetail,
    }

    #[derive(Debug, Clone, Deserialize)]
    pub struct ErrorDetail {
        pub message: String,
        #[serde(default)]
        pub r#type: Option<String>,
    }
}

pub async fn generate(
    client: &reqwest::Client,
    config: &super::Config,
    chat: &backends::Chat,
) -> super::Result<String> {
    let url = format!("{FEATHERLESS_API_URL_BASE}/chat/completions");

    let body = request::Request::new(config, chat);
    debug!(
        "Featherless chat completions request: {}",
        serde_json::to_string_pretty(&body).unwrap()
    );

    let response = client.post(url).json(&body).send().await?;

    if response.status() != StatusCode::OK {
        let error_response: response::ErrorResponse = response.json().await?;
        return Err(super::Error::Api {
            message: error_response.error.message,
        });
    }

    let response: response::ChatCompletion = response.json().await?;
    debug!("Featherless chat completions response: {:#?}", response);

    // Extract the content from the first choice.
    if let Some(choice) = response.choices.first() {
        Ok(choice.message.content.clone())
    } else {
        Err(super::Error::InvalidResponse(
            "No choices in response".to_string(),
        ))
    }
}
