//! Types and functions for the /messages endpoint.

use futures::stream::StreamExt;
use log::{info, trace};
use serde::{Deserialize, Serialize};
use sse_stream::SseStream;
use time::OffsetDateTime;

use super::{ANTHROPIC_API_URL_BASE, ANTHROPIC_API_VERSION};
use crate::data;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum Role {
    #[serde(rename = "assistant")]
    Assistant,
    #[serde(rename = "user")]
    User,
}

impl From<data::chat::Role> for Role {
    fn from(value: data::chat::Role) -> Self {
        match value {
            data::chat::Role::Assistant => Role::Assistant,
            data::chat::Role::User => Role::User,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MessagesRequest {
    max_tokens: u32,
    messages: Vec<InputMessage>,
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
            stream: true,
            system: String::new(),
        }
    }
}

impl From<&data::chat::Chat> for MessagesRequest {
    fn from(chat: &data::chat::Chat) -> Self {
        MessagesRequest {
            messages: chat.messages.iter().map(From::from).collect(),
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InputMessage {
    role: Role,
    content: String,
}

impl From<&data::chat::Message> for InputMessage {
    fn from(message: &data::chat::Message) -> Self {
        InputMessage {
            role: From::from(message.role),
            content: message.text.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MessagesResponse {}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ContentBlock {
    text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ResponseMessage {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StopReason {
    EndTurn,
    MaxTokens,
    StopSequence,
    ToolUse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Usage {
    input_tokens: Option<u32>,
    output_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MessageDelta {
    stop_reason: Option<StopReason>,
    stop_sequence: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
enum MessagesResponseEvent {
    MessageStart {
        message: ResponseMessage,
    },
    Ping,
    ContentBlockStart {
        index: u32,
        content_block: ContentBlock,
    },
    ContentBlockDelta {
        index: u32,
        delta: ContentBlock,
    },
    ContentBlockStop {
        index: u32,
    },
    MessageDelta {
        delta: MessageDelta,
        usage: Usage,
    },
    MessageStop,
}

//pub async fn generate(client: &reqwest::Client) -> super::Result<MessagesResponse> {}

pub async fn generate_stream(
    client: &reqwest::Client,
    chat: &data::chat::Chat,
) -> super::Result<String> {
    let url = format!("{ANTHROPIC_API_URL_BASE}/messages");

    let body = MessagesRequest::from(chat);
    let response = client.post(url).json(&body).send().await?;

    info!("Anthropic generate response code: {}", response.status());

    let mut sse_body = SseStream::from_byte_stream(response.bytes_stream());
    let mut generated_message = String::new();

    while let Some(sse) = sse_body.next().await {
        let sse = sse.unwrap();
        //debug!("{:?}", sse);

        let event: MessagesResponseEvent = serde_json::from_str(&sse.data.unwrap()).unwrap();
        debug!("{:?}", event);

        match event {
            MessagesResponseEvent::ContentBlockStart {
                index,
                content_block,
            } => generated_message.push_str(&content_block.text),
            MessagesResponseEvent::ContentBlockDelta { index, delta } => {
                generated_message.push_str(&delta.text)
            }
            _ => {}
        }
    }

    Ok(generated_message)
}
