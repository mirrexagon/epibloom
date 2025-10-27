//! Types and functions for the OpenAI-compatible chat completions endpoint.

use async_stream::try_stream;
use futures::stream::StreamExt;
use futures::Stream;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use sse_stream::SseStream;

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
        pub stream: bool,
        pub stream_options: Option<StreamOptions>,

        #[serde(skip_serializing_if = "Option::is_none")]
        pub temperature: Option<f32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub top_p: Option<f32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub top_k: Option<i32>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        pub stop: Vec<String>,
    }

    #[derive(Debug, Clone, Serialize)]
    pub struct StreamOptions {
        include_usage: bool,
    }

    impl Request {
        pub fn new(config: &super::super::Config, chat: &backends::Chat) -> Self {
            let stream_options = StreamOptions {
                include_usage: true,
            };

            Self {
                model: config.model.clone(),
                messages: chat.messages.iter().map(From::from).collect(),
                max_tokens: config.max_tokens,
                stream: true,
                stream_options: Some(stream_options),
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
    use uuid::Uuid;

    #[allow(dead_code)]
    #[derive(Debug, Clone, Deserialize)]
    pub struct Choice {
        pub index: u32,
        pub message: Message,
        pub finish_reason: String,
    }

    #[allow(dead_code)]
    #[derive(Debug, Clone, Deserialize)]
    pub struct Message {
        pub role: String,
        pub content: String,
    }

    #[allow(dead_code)]
    #[derive(Debug, Clone, Deserialize)]
    pub struct Usage {
        pub prompt_tokens: u32,
        pub completion_tokens: u32,
        pub total_tokens: u32,
    }

    #[allow(dead_code)]
    #[derive(Debug, Clone, Deserialize)]
    pub struct ErrorResponse {
        pub error: ErrorDetail,
    }

    #[allow(dead_code)]
    #[derive(Debug, Clone, Deserialize)]
    pub struct ErrorDetail {
        pub message: String,
        #[serde(default)]
        pub r#type: Option<String>,
    }

    #[allow(dead_code)]
    #[derive(Debug, Clone, Deserialize)]
    pub struct ChatCompletionChunk {
        pub id: Uuid,
        pub object: String,
        pub created: u64,
        pub model: String,
        pub choices: Vec<ChunkChoice>,
        pub usage: Option<Usage>,
    }

    #[allow(dead_code)]
    #[derive(Debug, Clone, Deserialize)]
    pub struct ChunkChoice {
        pub index: u32,
        pub delta: DeltaMessage,
        pub finish_reason: Option<String>,
    }

    #[allow(dead_code)]
    #[derive(Debug, Clone, Deserialize)]
    pub struct DeltaMessage {
        pub role: Option<String>,
        pub content: String,
    }
}

// TODO: Generalize to different thinking delimiter styles.
const THINKING_START_DELIMITER: &str = "<think>";
const THINKING_END_DELIMITER: &str = "</think>";

pub fn generate<'a>(
    client: &'a reqwest::Client,
    config: &'a super::Config,
    chat: &'a backends::Chat,
) -> impl Stream<Item = Result<backends::GenerateChunk, super::Error>> + 'a {
    try_stream! {
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
            })?;
        }

        let mut sse_body = SseStream::from_byte_stream(response.bytes_stream());

        let mut in_thinking = false;
        let mut skipping_whitespace = true;

        // From a basic text-only streaming session, we get:
        // - Lots of chunks with one choice with deltas with just content
        // - One with one choice with empty string content + role + finish reason
        // - One with no choices and with usage information and "system_fingerprint"
        // - Just the string "[DONE]"
        while let Some(sse) = sse_body.next().await {
            let sse = sse.unwrap(); // TODO: Propagate errors.

            let json_string = sse.data.unwrap();

            if json_string == "[DONE]" {
                break;
            }

            let chunk: response::ChatCompletionChunk = serde_json::from_str(&json_string).unwrap();

            if let Some(choice) = chunk.choices.first() {
                let mut text_delta = choice.delta.content.as_str();

                if skipping_whitespace {
                    text_delta = text_delta.trim();
                }

                if text_delta.is_empty() {
                    continue;
                } else {
                    skipping_whitespace = false;
                }

                // Look for thinking delimiters. We assume the entire delimiter
                // will be present in the delta because it is usually a single token.
                let delimiter_to_find = if in_thinking { THINKING_END_DELIMITER } else { THINKING_START_DELIMITER };
                if let Some((text_before, text_after)) = text_delta.split_once(delimiter_to_find) {
                    skipping_whitespace = false;

                    let (thinking_delta, message_delta) = if in_thinking {
                        (text_before, text_after)
                    } else {
                        (text_after, text_before)
                    };

                    if !thinking_delta.is_empty() {
                        yield backends::GenerateChunk::ThinkingDelta(thinking_delta.to_owned());
                    }

                    if !message_delta.is_empty() {
                        yield backends::GenerateChunk::MessageDelta(message_delta.to_owned());
                    }

                    in_thinking = !in_thinking;
                } else {
                    #[allow(clippy::collapsible_else_if)]
                    if in_thinking {
                        yield backends::GenerateChunk::ThinkingDelta(text_delta.to_owned());
                    } else {
                        yield backends::GenerateChunk::MessageDelta(text_delta.to_owned());
                    }
                }
            }
        }
    }
}
