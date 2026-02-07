//! Types and functions for the /messages endpoint.

use async_stream::try_stream;
use futures::Stream;
use futures::StreamExt;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use sse_stream::SseStream;

use super::ANTHROPIC_API_URL_BASE;
use crate::backend;
use crate::trace;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
enum Role {
    #[serde(rename = "assistant")]
    Assistant,
    #[serde(rename = "user")]
    User,
}

impl From<trace::Role> for Role {
    fn from(value: trace::Role) -> Self {
        match value {
            trace::Role::Assistant => Role::Assistant,
            trace::Role::User => Role::User,

            trace::Role::System => panic!("trace converted to Claude format had system messages - this should be prevented by a pre-processing step"),
        }
    }
}

#[allow(dead_code)]
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

    use crate::trace;

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
        pub fn new(config: &super::super::Config, trace: &trace::Trace) -> Self {
            Self {
                max_tokens: config.max_tokens,
                messages: trace.messages.iter().map(From::from).collect(),
                model: config.model.to_owned(),

                metadata: None,
                stop_sequences: if !config.stop_sequences.is_empty() {
                    Some(config.stop_sequences.clone())
                } else {
                    None
                },
                stream: true,
                system: None,
                temperature: None,
                thinking: config.thinking_budget.map(|num_tokens| Thinking::Enabled {
                    budget_tokens: num_tokens,
                }),
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

    impl From<&trace::Message> for Message {
        fn from(message: &trace::Message) -> Self {
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
        // TODO: Support adaptive and https://platform.claude.com/docs/en/build-with-claude/adaptive-thinking#adaptive-thinking-with-the-effort-parameter
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

    #[derive(Debug, Clone, Deserialize)]
    pub struct MessageDelta {
        stop_reason: Option<StopReason>,
        stop_sequence: Option<String>,
    }

    // TODO: Handle all the types here: https://docs.claude.com/en/docs/build-with-claude/streaming#content-block-delta-types
    // We need these variants to have these specific names. So we disable this
    // warning that complains about the common suffix.
    #[allow(clippy::enum_variant_names)]
    #[derive(Debug, Clone, Deserialize)]
    #[serde(rename_all = "snake_case", tag = "type")]
    pub enum ContentBlockDelta {
        TextDelta { text: String },
        InputJsonDelta { partial_json: String },
        ThinkingDelta { thinking: String },
        SignatureDelta { signature: String },
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
            content_block: super::ContentBlock,
        },
        ContentBlockDelta {
            index: u32,
            delta: ContentBlockDelta,
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
}

fn extract_system_prompt_from_trace(trace: &mut trace::Trace) -> Option<String> {
    let system_messages: Vec<_> = trace
        .messages
        .extract_if(.., |m| m.role == trace::Role::System)
        .map(|m| m.text)
        .collect();

    let system_prompt = system_messages.join("\n\n");

    if system_prompt.is_empty() {
        None
    } else {
        Some(system_prompt)
    }
}

pub fn generate(
    client: reqwest::Client,
    config: super::Config,
    mut trace: trace::Trace,
) -> impl Stream<Item = Result<backend::GenerateChunk, super::Error>> {
    try_stream! {
        let url = format!("{ANTHROPIC_API_URL_BASE}/messages");

        let system_prompt = extract_system_prompt_from_trace(&mut trace);
        let mut body = request::Request::new(&config, &trace);
        body.system = system_prompt;
        let request_json = serde_json::to_string_pretty(&body)?;
        debug!("Anthropic messages request: {}", request_json);

        let response = client.post(url).json(&body).send().await?;

        if response.status() != StatusCode::OK {
            let error_object: serde_json::Value = response.json().await?;
            return Err(super::Error::Api {
                r#type: error_object["error"]["type"].as_str().unwrap().to_owned(),
                message: error_object["error"]["message"]
                    .as_str()
                    .unwrap()
                    .to_owned(),
            })?;
        }

        let mut sse_body = SseStream::from_byte_stream(response.bytes_stream());

        while let Some(sse) = sse_body.next().await {
            let sse = sse?;

            let json_string = sse.data.ok_or(super::Error::StreamingEventHasNoData)?;

            let event: response::StreamingEvent = serde_json::from_str(&json_string)?;

            match event {
                response::StreamingEvent::MessageStart { message } => {
                    for content_block in message.content {
                        if let ContentBlock::Text { text, .. } = content_block {
                            yield backend::GenerateChunk::MessageDelta(text);
                        }
                    }
                }

                response::StreamingEvent::ContentBlockStart { content_block: ContentBlock::Text { text, .. }, .. } => {
                    yield backend::GenerateChunk::MessageDelta(text)
                }

                response::StreamingEvent::ContentBlockDelta { delta, .. } => {
                    match delta {
                        response::ContentBlockDelta::ThinkingDelta { thinking, .. } => yield backend::GenerateChunk::ThinkingDelta(thinking),
                        response::ContentBlockDelta::TextDelta { text, .. } => yield backend::GenerateChunk::MessageDelta(text),

                        _ => {}
                    }
                }

                _ => {}
            }
        }
    }
}
