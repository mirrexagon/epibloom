//! Types and functions for the /messages endpoint.

use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use super::ANTHROPIC_API_URL_BASE;
use crate::backends;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
enum Role {
    #[serde(rename = "assistant")]
    Assistant,
    #[serde(rename = "user")]
    User,
}

impl From<backends::Role> for Role {
    fn from(value: backends::Role) -> Self {
        match value {
            backends::Role::System => Role::User, // TODO: Concatenate all system messages into a single one that goes into the system message field.
            backends::Role::Assistant => Role::Assistant,
            backends::Role::User => Role::User,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
enum ContentBlock {
    Text {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        citations: Option<Citations>,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    Image {
        source: ImageSource,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    ToolUse {
        id: String,
        input: serde_json::Value,
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    Document {
        source: DocumentSource,
        citations: DocumentCitations,
        #[serde(skip_serializing_if = "Option::is_none")]
        context: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    Thinking {
        signature: String,
        thinking: String,
    },
    RedactedThinking {
        data: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
enum CacheControl {
    Ephemeral,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
enum Citations {
    CharLocation {
        cited_text: String,
        document_index: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        document_title: Option<String>,
        end_char_index: i32,
        start_char_index: u32,
    },
    PageLocation {
        cited_text: String,
        document_index: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        document_title: Option<String>,
        end_page_number: i32,
        start_page_number: u32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
enum ImageSource {
    Base64 {
        media_type: ImageSourceMediaType,
        data: String,
    },
    Url {
        url: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum ImageSourceMediaType {
    #[serde(rename = "image/jpeg")]
    Jpeg,
    #[serde(rename = "image/png")]
    Png,
    #[serde(rename = "image/gif")]
    Gif,
    #[serde(rename = "image/webp")]
    Webp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
enum DocumentSource {
    Base64 {
        media_type: Base64DocumentMediaType,
        data: String,
    },
    Text {
        media_type: TextDocumentMediaType,
        data: String,
    },
    Content {
        /// Only accepts Text or Image.
        content: Vec<ContentBlock>,
    },
    Url {
        url: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum Base64DocumentMediaType {
    #[serde(rename = "application/pdf")]
    Pdf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum TextDocumentMediaType {
    #[serde(rename = "text/plain")]
    Text,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DocumentCitations {
    pub enabled: bool,
}

#[allow(dead_code)]
mod request {
    use serde::Serialize;

    use crate::backends;

    #[derive(Debug, Clone, Serialize)]
    pub struct Request {
        pub max_tokens: u32,
        pub messages: Vec<Message>,
        pub model: String,

        #[serde(skip_serializing_if = "Option::is_none")]
        pub metadata: Option<Metadata>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub stop_sequences: Option<Vec<String>>,
        pub stream: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub system: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub temperature: Option<f32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub thinking: Option<Thinking>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub tool_choice: Option<ToolChoice>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub tools: Option<Vec<Tool>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub top_k: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub top_p: Option<f32>,
    }

    impl Request {
        pub fn new(config: &super::super::Config, chat: &backends::Chat) -> Self {
            Self {
                max_tokens: config.max_tokens,
                messages: chat.messages.iter().map(From::from).collect(),
                model: config.model.to_owned(),

                metadata: None,
                stop_sequences: if !config.stop_sequences.is_empty() {
                    Some(config.stop_sequences.clone())
                } else {
                    None
                },
                stream: false,
                system: None,
                temperature: None,
                thinking: if config.thinking {
                    Some(Thinking::Enabled {
                        budget_tokens: config.thinking_budget,
                    })
                } else {
                    None
                },
                tool_choice: None,
                tools: None,
                top_k: None,
                top_p: None,
            }
        }
    }

    #[derive(Debug, Clone, Serialize)]
    pub struct Message {
        pub role: super::Role,
        pub content: Vec<super::ContentBlock>,
    }

    impl From<&backends::Message> for Message {
        fn from(message: &backends::Message) -> Self {
            Message {
                role: From::from(message.role),
                content: vec![super::ContentBlock::Text {
                    text: message.text.clone(),
                    citations: None,
                    cache_control: None,
                }],
            }
        }
    }

    // TODO: impl From<response::Message> for Message
    // so that we can transform the response message into a request message to
    // send back to the model?
    // But this will be done through backends always anyway.

    impl Default for Message {
        fn default() -> Self {
            Self {
                role: super::Role::Assistant,
                content: Vec::new(),
            }
        }
    }

    #[derive(Debug, Clone, Serialize)]
    pub struct Metadata {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub user_id: Option<String>,
    }

    #[derive(Debug, Clone, Serialize)]
    #[serde(rename_all = "snake_case", tag = "type")]
    pub enum Thinking {
        Disabled,
        Enabled { budget_tokens: u32 },
    }

    #[derive(Debug, Clone, Serialize)]
    #[serde(rename_all = "snake_case", tag = "type")]
    pub enum ToolChoice {
        Auto {
            disable_parallel_tool_use: bool,
        },
        Any {
            disable_parallel_tool_use: bool,
        },
        Tool {
            name: String,
            disable_parallel_tool_use: bool,
        },
        None,
    }

    #[derive(Debug, Clone, Serialize)]
    #[serde(rename_all = "snake_case", tag = "type")]
    pub enum Tool {
        Custom {
            name: String,
            description: String,
            input_schema: serde_json::Value,
        },
    }
}

#[allow(dead_code)]
mod response {
    use serde::Deserialize;

    #[derive(Debug, Clone, Deserialize)]
    pub struct Message {
        pub role: super::Role,
        pub content: Vec<super::ContentBlock>,

        pub id: String,
        pub model: String,
    }

    #[derive(Debug, Clone, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum StopReason {
        EndTurn,
        MaxTokens,
        StopSequence,
        ToolUse,
        PauseTurn,
        Refusal,
    }

    #[derive(Debug, Clone, Deserialize)]
    pub struct Usage {
        pub input_tokens: Option<u32>,
        pub output_tokens: u32,
    }

    /*
    #[derive(Debug, Clone, Deserialize)]
    pub struct MessageDelta {
        stop_reason: Option<StopReason>,
        stop_sequence: Option<String>,
    }

    #[derive(Debug, Clone, Deserialize)]
    #[serde(rename_all = "snake_case", tag = "type")]
    pub enum StreamingEvent {
        MessageStart {
            message: Message,
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
    */
}

pub async fn generate(
    client: &reqwest::Client,
    config: &super::Config,
    chat: &backends::Chat,
) -> super::Result<String> {
    let url = format!("{ANTHROPIC_API_URL_BASE}/messages");

    let body = request::Request::new(config, chat);
    debug!(
        "Anthropic messages request: {}",
        serde_json::to_string_pretty(&body).unwrap()
    );

    let response = client.post(url).json(&body).send().await?;

    if response.status() != StatusCode::OK {
        let error_object: serde_json::Value = response.json().await?;
        return Err(super::Error::Api {
            r#type: error_object["error"]["type"].as_str().unwrap().to_owned(),
            message: error_object["error"]["message"]
                .as_str()
                .unwrap()
                .to_owned(),
        });
    }

    let response: response::Message = response.json().await?;
    debug!("Anthropic messages response: {:#?}", response);

    let mut result = String::new();
    for block in response.content.iter() {
        if let ContentBlock::Text { text, .. } = block {
            result.push_str(text);
        }
    }

    Ok(result)
}

// TODO: Return a MessagesResponse built up from the events to start with, since
// the point of the deltas and such is to fill in a complete response.
/*
pub async fn generate_stream(
    client: &reqwest::Client,
    chat: &backends::Chat
) -> super::Result<String> {
    let url = format!("{ANTHROPIC_API_URL_BASE}/messages");

    let body = request::Request::from(chat);
    let response = client.post(url).json(&body).send().await?;

    info!("Anthropic generate response code: {}", response.status());

    let mut sse_body = SseStream::from_byte_stream(response.bytes_stream());
    let mut generated_message = String::new();

    while let Some(sse) = sse_body.next().await {
        let sse = sse.unwrap();
        //debug!("{:#?}", sse);

        let event: MessagesResponseEvent = serde_json::from_str(&sse.data.unwrap()).unwrap();
        debug!("{:#?}", event);

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
*/
