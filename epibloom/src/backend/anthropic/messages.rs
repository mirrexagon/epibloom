//! Types and functions for the /messages endpoint.

use async_stream::try_stream;
use futures::Stream;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;
use sse_stream::SseStream;

use super::AnthropicReqwestClient;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum Role {
    #[serde(rename = "assistant")]
    Assistant,
    #[serde(rename = "user")]
    User,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum ContentBlock {
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
pub enum CacheControl {
    Ephemeral,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum Citations {
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
pub enum ImageSource {
    Base64 {
        media_type: ImageSourceMediaType,
        data: String,
    },
    Url {
        url: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImageSourceMediaType {
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
pub enum DocumentSource {
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
pub enum Base64DocumentMediaType {
    #[serde(rename = "application/pdf")]
    Pdf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TextDocumentMediaType {
    #[serde(rename = "text/plain")]
    Text,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentCitations {
    pub enabled: bool,
}

#[allow(dead_code)]
pub mod request {
    use serde::{Deserialize, Serialize};
    use serde_with::skip_serializing_none;

    use crate::backend::anthropic::messages::CacheControl;

    #[skip_serializing_none]
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MessagesRequest {
        pub max_tokens: u32,
        pub messages: Vec<Message>,
        pub model: String,

        pub metadata: Option<Metadata>,
        pub stop_sequences: Option<Vec<String>>,
        #[serde(default = "default_stream")]
        pub stream: bool,
        pub system: Option<String>,
        pub temperature: Option<f32>,
        pub thinking: Option<ThinkingConfig>,
        pub tool_choice: Option<ToolChoice>,
        pub tools: Option<Vec<Tool>>,
        pub top_k: Option<u32>,
        pub top_p: Option<f32>,
        // TODO: https://platform.claude.com/docs/en/build-with-claude/structured-outputs
        // Abstract the ability to put in structured outputs? XML vs. JSON, etc. How to bind this to Lua nicely?
    }

    fn default_stream() -> bool {
        true
    }

    #[skip_serializing_none]
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct CountTokensRequest {
        pub messages: Vec<Message>,
        pub model: String,
        pub cache_control: Option<CacheControl>,
        pub system: Option<String>,
        pub thinking: Option<ThinkingConfig>,
        pub tool_choice: Option<ToolChoice>,
        pub tools: Option<Vec<Tool>>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Message {
        pub role: super::Role,
        pub content: MessageContent,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(untagged)]
    pub enum MessageContent {
        String(String),
        ContentBlocks(Vec<super::ContentBlock>),
    }

    impl Default for Message {
        fn default() -> Self {
            Self {
                role: super::Role::User,
                content: MessageContent::ContentBlocks(Vec::new()),
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Metadata {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub user_id: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case", tag = "type")]
    pub enum ThinkingConfig {
        Disabled,
        Enabled { budget_tokens: u32 },
        // TODO: Support adaptive and https://platform.claude.com/docs/en/build-with-claude/adaptive-thinking#adaptive-thinking-with-the-effort-parameter
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
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

    #[derive(Debug, Clone, Serialize, Deserialize)]
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
pub mod response {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Response {
        // TODO: Have a function that takes a streaming event and updates the
        // response such that the final response given all streaming events in
        // order matches what you would've gotten without streaming.
        // Have a test in the parent module to attempt to test this, or at least
        // run streaming and non-streaming side by side and check the responses.
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
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

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Message {
        pub role: super::Role,
        pub content: Vec<super::ContentBlock>,

        pub id: String,
        pub model: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum StopReason {
        EndTurn,
        MaxTokens,
        StopSequence,
        ToolUse,
        PauseTurn,
        Refusal,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Usage {
        pub input_tokens: u32,
        pub output_tokens: u32,
        pub cache_creation_input_tokens: u32,
        pub cache_read_input_tokens: u32,
        pub inference_geo: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MessageDelta {
        stop_reason: Option<StopReason>,
        stop_sequence: Option<String>,
    }

    // TODO: Handle all the types here: https://docs.claude.com/en/docs/build-with-claude/streaming#content-block-delta-types
    // We need these variants to have these specific names. So we disable this
    // warning that complains about the common suffix.
    #[allow(clippy::enum_variant_names)]
    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case", tag = "type")]
    pub enum ContentBlockDelta {
        TextDelta { text: String },
        InputJsonDelta { partial_json: String },
        ThinkingDelta { thinking: String },
        SignatureDelta { signature: String },
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MessageTokensCount {
        pub input_tokens: u32,
    }
}

pub fn generate(
    client: reqwest::Client,
    request: request::MessagesRequest,
) -> impl Stream<Item = Result<response::StreamingEvent, super::Error>> {
    try_stream! {
        let request_json = serde_json::to_string_pretty(&request)?;
        log::debug!("Anthropic messages request: {}", request_json);

        let response = client.anthropic_post("messages", &request).await?;

        // TODO: Handle stream being true or false, return events or whole response.
        // Probably have separate functions that just override the `stream` request field.

        let mut sse_body = SseStream::from_byte_stream(response.bytes_stream());

        while let Some(sse) = sse_body.next().await {
            let sse = sse?;

            let json_string = sse.data.ok_or(super::Error::StreamingEventHasNoData)?;

            let event: response::StreamingEvent = serde_json::from_str(&json_string)?;
            log::debug!("Anthropic streaming event: {:?}", event);

            yield event;
        }
    }
}

pub async fn count_tokens(
    client: reqwest::Client,
    request: request::CountTokensRequest,
) -> Result<response::MessageTokensCount, super::Error> {
    let request_json = serde_json::to_string_pretty(&request)?;
    log::debug!("Anthropic messages token count request: {}", request_json);

    let response = client
        .anthropic_post("messages/count_tokens", &request)
        .await?;
    let result = response.json().await?;
    log::debug!("Token count response: {result:?}");

    Ok(result)
}
