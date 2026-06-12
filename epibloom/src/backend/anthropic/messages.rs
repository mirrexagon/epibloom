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
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum ContentBlock {
    Text {
        text: String,
        citations: Option<Citations>,
        cache_control: Option<CacheControl>,
    },
    Image {
        source: ImageSource,
        cache_control: Option<CacheControl>,
    },
    ToolUse {
        id: String,
        input: serde_json::Value,
        name: String,
        cache_control: Option<CacheControl>,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
        cache_control: Option<CacheControl>,
    },
    Document {
        source: DocumentSource,
        citations: DocumentCitations,
        context: Option<String>,
        title: Option<String>,
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
pub struct CacheControl {
    r#type: CacheControlType,
    ttl: Option<CacheControlTtl>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CacheControlType {
    Ephemeral,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CacheControlTtl {
    #[serde(rename = "5m")]
    FiveMinutes,
    #[serde(rename = "1h")]
    OneHour,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum Citations {
    CharLocation {
        cited_text: String,
        document_index: u32,
        document_title: Option<String>,
        end_char_index: i32,
        start_char_index: u32,
    },
    PageLocation {
        cited_text: String,
        document_index: u32,
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
        /// The maximum number of tokens to generate before stopping.
        pub max_tokens: u32,

        /// Input messages.
        pub messages: Vec<Message>,

        /// The model that will complete your prompt.
        pub model: String,

        /// Top-level cache control automatically applies a cache_control marker
        /// to the last cacheable block in the request.
        pub cache_control: Option<CacheControl>,

        /// Container identifier for reuse across requests.
        pub container: Option<String>,

        /// Specifies the geographic region for inference processing. If not
        /// specified, the workspace's `default_inference_geo` is used.
        pub inference_geo: Option<String>,

        /// An object describing metadata about the request.
        pub metadata: Option<Metadata>,

        /// Configuration options for the model's output, such as the output
        /// format.
        pub output_config: Option<OutputConfig>,

        /// Determines whether to use priority capacity (if available) or standard capacity for this request.
        ///
        /// Anthropic offers different levels of service for your API requests. See
        /// [service-tiers](https://platform.claude.com/docs/en/api/service-tiers)
        /// for details.
        pub service_tier: Option<ServiceTier>,

        /// Custom text sequences that will cause the model to stop generating.
        pub stop_sequences: Option<Vec<String>>,

        /// Whether to incrementally stream the response using server-sent events.
        pub stream: Option<bool>,

        /// System prompt.
        pub system: Option<String>,

        /// Amount of randomness injected into the response.
        ///
        /// Defaults to 1.0. Ranges from 0.0 to 1.0. Use temperature closer to
        /// 0.0 for analytical / multiple choice, and closer to 1.0 for creative
        /// and generative tasks.
        ///
        /// Note that even with temperature of 0.0, the results will not be
        /// fully deterministic.
        pub temperature: Option<f32>,

        /// Configuration for enabling Claude's extended thinking.
        ///
        /// When enabled, responses include thinking content blocks showing
        /// Claude's thinking process before the final answer. Requires a
        /// minimum budget of 1,024 tokens and counts towards your max_tokens
        /// limit.
        ///
        /// See [extended thinking](https://platform.claude.com/docs/en/build-with-claude/extended-thinking)
        /// for details.
        pub thinking: Option<ThinkingConfig>,

        pub tool_choice: Option<ToolChoice>,
        pub tools: Option<Vec<Tool>>,

        /// Only sample from the top K options for each subsequent token.
        ///
        /// Used to remove "long tail" low probability responses.
        pub top_k: Option<u32>,

        /// Use nucleus sampling.
        ///
        /// In nucleus sampling, we compute the cumulative distribution over
        /// all the options for each subsequent token in decreasing probability
        /// order and cut it off once it reaches a particular probability
        /// specified by top_p.
        pub top_p: Option<f32>,
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

    #[skip_serializing_none]
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Metadata {
        pub user_id: Option<String>,
    }

    #[skip_serializing_none]
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct OutputConfig {
        pub effort: Option<Effort>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum ServiceTier {
        Auto,
        StandardOnly,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum Effort {
        Low,
        Medium,
        High,
        Xhigh,
        Max,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case", tag = "type")]
    pub enum JsonOutputFormat {
        JsonSchema { schema: serde_json::Value },
    }

    #[skip_serializing_none]
    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case", tag = "type")]
    pub enum ThinkingConfig {
        Disabled,
        Enabled {
            /// Determines how many tokens Claude can use for its internal
            /// reasoning process. Larger budgets can enable more thorough
            /// analysis for complex problems, improving response quality.
            budget_tokens: u32,

            /// Controls how thinking content appears in the response. When set
            /// to [ThinkingDisplay::Summarized], thinking is returned normally.
            /// When set to [ThinkingDisplay::Omitted], thinking content is
            /// redacted but a signature is returned for multi-turn continuity.
            /// Defaults to [ThinkingDisplay::Summarized].
            display: Option<ThinkingDisplay>,
        },
        Adaptive {
            /// Controls how thinking content appears in the response. When set
            /// to [ThinkingDisplay::Summarized], thinking is returned normally.
            /// When set to [ThinkingDisplay::Omitted], thinking content is
            /// redacted but a signature is returned for multi-turn continuity.
            /// Defaults to [ThinkingDisplay::Summarized].
            display: Option<ThinkingDisplay>,
        },
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum ThinkingDisplay {
        Summarized,
        Omitted,
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

    #[skip_serializing_none]
    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case", tag = "type")]
    pub enum Tool {
        Custom {
            name: String,
            description: String,
            input_schema: serde_json::Value,
            cache_control: Option<CacheControl>,
            defer_loading: Option<bool>,
            eager_input_streaming: Option<bool>,
            input_examples: Vec<serde_json::Value>,
            strict: Option<bool>,
        },
    }
}

#[allow(dead_code)]
pub mod response {
    use serde::{Deserialize, Serialize};
    use serde_with::skip_serializing_none;

    // TODO: Have a function that takes a streaming event and updates the
    // response such that the final response given all streaming events in
    // order matches what you would've gotten without streaming.
    // Have a test in the parent module to attempt to test this, or at least
    // run streaming and non-streaming side by side and check the responses.
    #[skip_serializing_none]
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Message {
        pub id: String,
        pub model: String,
        pub role: super::Role,
        pub content: Vec<super::ContentBlock>,

        pub stop_reason: Option<StopReason>,
        pub stop_details: Option<StopDetails>,
        pub stop_sequence: Option<String>,

        pub usage: Usage,

        pub container: Option<Container>,
    }

    impl Message {
        pub fn apply_event(&mut self, event: MessageStreamEvent) {
            // TODO: Implement and bind to Lua
            unimplemented!();
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case", tag = "type")]
    pub enum MessageStreamEvent {
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
    pub struct Container {
        id: String,
        expires_at: String,
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
    #[serde(rename_all = "snake_case", tag = "type")]
    pub enum StopDetails {
        Refusal {
            category: String,
            explanation: String,
        },
    }

    #[skip_serializing_none]
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Usage {
        pub input_tokens: u32,
        pub cache_creation_input_tokens: u32,
        pub cache_read_input_tokens: u32,
        pub cache_creation: Option<CacheCreationUsageBreakdown>,

        pub output_tokens: u32,
        pub output_tokens_details: Option<OutputTokensDetails>,

        pub server_tool_use: Option<ServerToolUsage>,

        pub service_tier: Option<ServiceTier>,

        pub inference_geo: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct CacheCreationUsageBreakdown {
        ephemeral_5m_input_tokens: u32,
        ephemeral_1h_input_tokens: u32,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct OutputTokensDetails {
        thinking_tokens: u32,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ServerToolUsage {
        web_fetch_requests: u32,
        web_search_requests: u32,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum ServiceTier {
        Standard,
        Priority,
        Batch,
    }

    #[skip_serializing_none]
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

pub async fn generate(
    client: reqwest::Client,
    mut request: request::MessagesRequest,
) -> Result<response::Message, super::Error> {
    request.stream = Some(false);

    let request_json = serde_json::to_string_pretty(&request)?;
    log::debug!("Anthropic messages request: {request_json}");

    let response = client.anthropic_post("messages", &request).await?;
    let response_message: serde_json::Value = response.json().await?;
    log::debug!("Anthropic messages raw response: {response_message:#?}");
    Ok(serde_json::from_value(response_message)?)
}

pub fn generate_streaming(
    client: reqwest::Client,
    mut request: request::MessagesRequest,
) -> impl Stream<Item = Result<response::MessageStreamEvent, super::Error>> {
    try_stream! {
        request.stream = Some(true);

        let request_json = serde_json::to_string_pretty(&request)?;
        log::debug!("Anthropic messages request: {}", request_json);

        let response = client.anthropic_post("messages", &request).await?;

        let mut sse_body = SseStream::from_byte_stream(response.bytes_stream());

        while let Some(sse) = sse_body.next().await {
            let sse = sse?;

            let json_string = sse.data.ok_or(super::Error::StreamingEventHasNoData)?;
            log::debug!("Anthropic streaming event JSON: {:#?}", serde_json::from_str::<serde_json::Value>(&json_string).unwrap());

            let event: response::MessageStreamEvent = serde_json::from_str(&json_string)?;
            log::debug!("Anthropic streaming event: {event:?}");

            yield event;
        }
    }
}

pub async fn count_tokens(
    client: reqwest::Client,
    request: request::CountTokensRequest,
) -> Result<response::MessageTokensCount, super::Error> {
    let request_json = serde_json::to_string_pretty(&request)?;
    log::debug!("Anthropic messages token count request: {request_json}");

    let response = client
        .anthropic_post("messages/count_tokens", &request)
        .await?;
    let response_token_count = response.json().await?;
    log::debug!("Token count response: {response_token_count:?}");
    Ok(response_token_count)
}
