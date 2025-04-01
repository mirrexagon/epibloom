//! Types and functions for the /messages endpoint.

use futures::stream::StreamExt;
use log::info;
use serde::{Deserialize, Serialize};
use sse_stream::SseStream;

use super::ANTHROPIC_API_URL_BASE;
use crate::data;

mod request {
    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct Request {
        max_tokens: u32,
        messages: Vec<Message>,
        model: String,

        metadata: Option<Metadata>,
        stop_sequences: Option<Vec<String>>,
        stream: bool,
        system: Option<String>,
        temperature: Option<f32>,
        thinking: Option<Thinking>,
        tool_choice: Option<ToolChoice>,
        tools: Vec<Tool>,
        top_k: Option<u32>,
        top_p: Option<f32>,
    }

    impl Default for Request {
        fn default() -> Self {
            Request {
                max_tokens: 256,
                messages: Vec::new(),
                model: "claude-3-haiku-20240307".to_string(),

                metadata: None,
                stop_sequences: None,
                stream: false,
                system: None,
            }
        }
    }

    impl From<&data::chat::Chat> for Request {
        fn from(chat: &data::chat::Chat) -> Self {
            Request {
                messages: chat.messages.iter().map(From::from).collect(),
                ..Default::default()
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct Message {
        role: Role,
        content: Vec<ContentBlock>,
    }

    impl From<&data::chat::Message> for Message {
        fn from(message: &data::chat::Message) -> Self {
            Message {
                role: From::from(message.role),
                content: ContentBlock::Text {
                    text: message.text.clone(),
                    ..Default::default()
                },
            }
        }
    }

    impl Default for Message {
        fn default() -> Self {
            Self {
                role: Role::Assistant,
                content: Vec::new(),
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case", tag = "type")]
    enum ContentBlock {
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
        // TODO: More
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
        enabled: bool,
    }

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
    struct Metadata {
        user_id: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case", tag = "type")]
    enum Thinking {
        Disabled,
        Enabled { budget_tokens: u32 },
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case", tag = "type")]
    enum ToolChoice {
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
    enum Tool {
        Custom {
            name: String,
            description: String,
            input_schema: serde_json::Value,
        },
    }
}

mod response {
    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct MessagesResponse {}

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
}

//pub async fn generate(client: &reqwest::Client) -> super::Result<MessagesResponse> {}

// TODO: Return a MessagesResponse built up from the events to start with, since
// the point of the deltas and such is to fill in a complete response.
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
