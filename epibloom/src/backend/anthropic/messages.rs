//! Types and functions for the /messages endpoint.
//!
//! This module closely follows the Anthropic Messages API documentation,
//! using canonical type names and separating input parameters from output types.

use async_stream::try_stream;
use futures::Stream;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;
use sse_stream::SseStream;

use super::AnthropicReqwestClient;

// ============================================================================
// Common Types (used in both input and output)
// ============================================================================

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CacheControlEphemeral {
    #[serde(rename = "ephemeral")]
    Ephemeral {
        #[serde(skip_serializing_if = "Option::is_none")]
        ttl: Option<CacheControlTtl>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CacheControlTtl {
    #[serde(rename = "5m")]
    FiveMinutes,
    #[serde(rename = "1h")]
    OneHour,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImageMediaType {
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
pub enum PdfMediaType {
    #[serde(rename = "application/pdf")]
    Pdf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TextMediaType {
    #[serde(rename = "text/plain")]
    Plain,
}

// ============================================================================
// Input Types (Request Parameters)
// ============================================================================

/// Input message parameter.
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageParam {
    pub role: Role,
    pub content: MessageContentParam,
}

impl From<Message> for MessageParam {
    fn from(message: Message) -> Self {
        Self {
            role: message.role,
            content: MessageContentParam::Blocks(
                message
                    .content
                    .into_iter()
                    .filter_map(|block| match block {
                        ContentBlock::Text { text, citations } => Some(ContentBlockParam::Text {
                            text,
                            cache_control: None,
                            citations: citations.map(|cits| {
                                cits.into_iter()
                                    .map(|cit| match cit {
                                        TextCitation::CharLocation {
                                            cited_text,
                                            document_index,
                                            document_title,
                                            start_char_index,
                                            end_char_index,
                                            file_id: _,
                                        } => TextCitationParam::CharLocation {
                                            cited_text,
                                            document_index,
                                            document_title,
                                            start_char_index,
                                            end_char_index,
                                        },
                                        TextCitation::PageLocation {
                                            cited_text,
                                            document_index,
                                            document_title,
                                            start_page_number,
                                            end_page_number,
                                            file_id: _,
                                        } => TextCitationParam::PageLocation {
                                            cited_text,
                                            document_index,
                                            document_title,
                                            start_page_number,
                                            end_page_number,
                                        },
                                        TextCitation::ContentBlockLocation {
                                            cited_text,
                                            document_index,
                                            document_title,
                                            start_block_index,
                                            end_block_index,
                                            file_id: _,
                                        } => TextCitationParam::ContentBlockLocation {
                                            cited_text,
                                            document_index,
                                            document_title,
                                            start_block_index,
                                            end_block_index,
                                        },
                                        TextCitation::WebSearchResultLocation {
                                            cited_text,
                                            encrypted_index,
                                            title,
                                            url,
                                        } => TextCitationParam::WebSearchResultLocation {
                                            cited_text,
                                            encrypted_index,
                                            title,
                                            url,
                                        },
                                        TextCitation::SearchResultLocation {
                                            cited_text,
                                            title,
                                            source,
                                            search_result_index,
                                            start_block_index,
                                            end_block_index,
                                        } => TextCitationParam::SearchResultLocation {
                                            cited_text,
                                            title,
                                            source,
                                            search_result_index,
                                            start_block_index,
                                            end_block_index,
                                        },
                                    })
                                    .collect()
                            }),
                        }),
                        ContentBlock::Thinking { signature, thinking } => {
                            Some(ContentBlockParam::Thinking { signature, thinking })
                        }
                        ContentBlock::RedactedThinking { data } => {
                            Some(ContentBlockParam::RedactedThinking { data })
                        }
                        ContentBlock::ToolUse {
                            id,
                            name,
                            input,
                            caller,
                        } => Some(ContentBlockParam::ToolUse {
                            id,
                            name,
                            input,
                            cache_control: None,
                            caller,
                        }),
                        // Skip blocks that are typically not sent back in subsequent requests
                        _ => None,
                    })
                    .collect(),
            ),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContentParam {
    Text(String),
    Blocks(Vec<ContentBlockParam>),
}

#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlockParam {
    #[serde(rename = "text")]
    Text {
        text: String,
        cache_control: Option<CacheControlEphemeral>,
        citations: Option<Vec<TextCitationParam>>,
    },
    #[serde(rename = "image")]
    Image {
        source: ImageSourceParam,
        cache_control: Option<CacheControlEphemeral>,
    },
    #[serde(rename = "document")]
    Document {
        source: DocumentSourceParam,
        cache_control: Option<CacheControlEphemeral>,
        citations: Option<CitationsConfigParam>,
        context: Option<String>,
        title: Option<String>,
    },
    #[serde(rename = "search_result")]
    SearchResult {
        content: Vec<TextBlockParam>,
        source: String,
        title: String,
        cache_control: Option<CacheControlEphemeral>,
        citations: Option<CitationsConfigParam>,
    },
    #[serde(rename = "thinking")]
    Thinking { signature: String, thinking: String },
    #[serde(rename = "redacted_thinking")]
    RedactedThinking { data: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
        cache_control: Option<CacheControlEphemeral>,
        caller: Option<ToolCaller>,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: Option<ToolResultContentParam>,
        is_error: Option<bool>,
        cache_control: Option<CacheControlEphemeral>,
    },
    #[serde(rename = "server_tool_use")]
    ServerToolUse {
        id: String,
        name: ServerToolName,
        input: serde_json::Value,
        cache_control: Option<CacheControlEphemeral>,
        caller: Option<ToolCaller>,
    },
    #[serde(rename = "web_search_tool_result")]
    WebSearchToolResult {
        tool_use_id: String,
        content: WebSearchToolResultContentParam,
        cache_control: Option<CacheControlEphemeral>,
        caller: Option<ToolCaller>,
    },
    #[serde(rename = "web_fetch_tool_result")]
    WebFetchToolResult {
        tool_use_id: String,
        content: WebFetchToolResultContentParam,
        cache_control: Option<CacheControlEphemeral>,
        caller: Option<ToolCaller>,
    },
    #[serde(rename = "code_execution_tool_result")]
    CodeExecutionToolResult {
        tool_use_id: String,
        content: CodeExecutionToolResultContentParam,
        cache_control: Option<CacheControlEphemeral>,
    },
    #[serde(rename = "bash_code_execution_tool_result")]
    BashCodeExecutionToolResult {
        tool_use_id: String,
        content: BashCodeExecutionToolResultContentParam,
        cache_control: Option<CacheControlEphemeral>,
    },
    #[serde(rename = "text_editor_code_execution_tool_result")]
    TextEditorCodeExecutionToolResult {
        tool_use_id: String,
        content: TextEditorCodeExecutionToolResultContentParam,
        cache_control: Option<CacheControlEphemeral>,
    },
    #[serde(rename = "tool_search_tool_result")]
    ToolSearchToolResult {
        tool_use_id: String,
        content: ToolSearchToolResultContentParam,
        cache_control: Option<CacheControlEphemeral>,
    },
    #[serde(rename = "container_upload")]
    ContainerUpload {
        file_id: String,
        cache_control: Option<CacheControlEphemeral>,
    },
    #[serde(rename = "mid_conv_system")]
    MidConvSystem {
        content: Vec<TextBlockParam>,
        cache_control: Option<CacheControlEphemeral>,
    },
}

/// Text block parameter (used in various contexts).
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextBlockParam {
    pub text: String,
    #[serde(rename = "type")]
    pub type_: TextBlockType,
    pub cache_control: Option<CacheControlEphemeral>,
    pub citations: Option<Vec<TextCitationParam>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TextBlockType {
    #[serde(rename = "text")]
    Text,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TextCitationParam {
    #[serde(rename = "char_location")]
    CharLocation {
        cited_text: String,
        document_index: u32,
        document_title: String,
        start_char_index: u32,
        end_char_index: u32,
    },
    #[serde(rename = "page_location")]
    PageLocation {
        cited_text: String,
        document_index: u32,
        document_title: String,
        start_page_number: u32,
        end_page_number: u32,
    },
    #[serde(rename = "content_block_location")]
    ContentBlockLocation {
        cited_text: String,
        document_index: u32,
        document_title: String,
        start_block_index: u32,
        end_block_index: u32,
    },
    #[serde(rename = "web_search_result_location")]
    WebSearchResultLocation {
        cited_text: String,
        encrypted_index: String,
        title: String,
        url: String,
    },
    #[serde(rename = "search_result_location")]
    SearchResultLocation {
        cited_text: String,
        title: String,
        source: String,
        search_result_index: u32,
        start_block_index: u32,
        end_block_index: u32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ImageSourceParam {
    #[serde(rename = "base64")]
    Base64 {
        media_type: ImageMediaType,
        data: String,
    },
    #[serde(rename = "url")]
    Url { url: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DocumentSourceParam {
    #[serde(rename = "base64")]
    Base64 {
        media_type: PdfMediaType,
        data: String,
    },
    #[serde(rename = "text")]
    Text {
        media_type: TextMediaType,
        data: String,
    },
    #[serde(rename = "content")]
    Content {
        content: DocumentSourceContentParam,
    },
    #[serde(rename = "url")]
    Url { url: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DocumentSourceContentParam {
    Text(String),
    Blocks(Vec<DocumentContentBlockParam>),
}

#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DocumentContentBlockParam {
    #[serde(rename = "text")]
    Text {
        text: String,
        cache_control: Option<CacheControlEphemeral>,
        citations: Option<Vec<TextCitationParam>>,
    },
    #[serde(rename = "image")]
    Image {
        source: ImageSourceParam,
        cache_control: Option<CacheControlEphemeral>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CitationsConfigParam {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ToolCaller {
    #[serde(rename = "direct")]
    Direct,
    #[serde(rename = "code_execution_20250825")]
    CodeExecution20250825 { tool_id: String },
    #[serde(rename = "code_execution_20260120")]
    CodeExecution20260120 { tool_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServerToolName {
    WebSearch,
    WebFetch,
    CodeExecution,
    BashCodeExecution,
    TextEditorCodeExecution,
    ToolSearchToolRegex,
    ToolSearchToolBm25,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolResultContentParam {
    Text(String),
    Blocks(Vec<ToolResultContentBlockParam>),
}

#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ToolResultContentBlockParam {
    #[serde(rename = "text")]
    Text {
        text: String,
        cache_control: Option<CacheControlEphemeral>,
        citations: Option<Vec<TextCitationParam>>,
    },
    #[serde(rename = "image")]
    Image {
        source: ImageSourceParam,
        cache_control: Option<CacheControlEphemeral>,
    },
    #[serde(rename = "search_result")]
    SearchResult {
        content: Vec<TextBlockParam>,
        source: String,
        title: String,
        cache_control: Option<CacheControlEphemeral>,
        citations: Option<CitationsConfigParam>,
    },
    #[serde(rename = "document")]
    Document {
        source: DocumentSourceParam,
        cache_control: Option<CacheControlEphemeral>,
        citations: Option<CitationsConfigParam>,
        context: Option<String>,
        title: Option<String>,
    },
    #[serde(rename = "tool_reference")]
    ToolReference {
        tool_name: String,
        cache_control: Option<CacheControlEphemeral>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WebSearchToolResultContentParam {
    Results(Vec<WebSearchResultBlockParam>),
    Error(WebSearchToolResultErrorParam),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearchResultBlockParam {
    pub encrypted_content: String,
    pub title: String,
    #[serde(rename = "type")]
    pub type_: WebSearchResultBlockType,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_age: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WebSearchResultBlockType {
    #[serde(rename = "web_search_result")]
    WebSearchResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearchToolResultErrorParam {
    pub error_code: WebSearchToolResultErrorCode,
    #[serde(rename = "type")]
    pub type_: WebSearchToolResultErrorType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WebSearchToolResultErrorType {
    #[serde(rename = "web_search_tool_result_error")]
    WebSearchToolResultError,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebSearchToolResultErrorCode {
    InvalidToolInput,
    Unavailable,
    MaxUsesExceeded,
    TooManyRequests,
    QueryTooLong,
    RequestTooLarge,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WebFetchToolResultContentParam {
    Result(WebFetchBlockParam),
    Error(WebFetchToolResultErrorBlockParam),
}

#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebFetchBlockParam {
    pub content: DocumentBlockParam,
    #[serde(rename = "type")]
    pub type_: WebFetchBlockType,
    pub url: String,
    pub retrieved_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WebFetchBlockType {
    #[serde(rename = "web_fetch_result")]
    WebFetchResult,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentBlockParam {
    pub source: DocumentSourceParam,
    #[serde(rename = "type")]
    pub type_: DocumentBlockType,
    pub cache_control: Option<CacheControlEphemeral>,
    pub citations: Option<CitationsConfigParam>,
    pub context: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DocumentBlockType {
    #[serde(rename = "document")]
    Document,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebFetchToolResultErrorBlockParam {
    pub error_code: WebFetchToolResultErrorCode,
    #[serde(rename = "type")]
    pub type_: WebFetchToolResultErrorType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WebFetchToolResultErrorType {
    #[serde(rename = "web_fetch_tool_result_error")]
    WebFetchToolResultError,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebFetchToolResultErrorCode {
    InvalidToolInput,
    UrlTooLong,
    UrlNotAllowed,
    UrlNotInPriorContext,
    UrlNotAccessible,
    UnsupportedContentType,
    TooManyRequests,
    MaxUsesExceeded,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CodeExecutionToolResultContentParam {
    Result(CodeExecutionResultBlockParam),
    EncryptedResult(EncryptedCodeExecutionResultBlockParam),
    Error(CodeExecutionToolResultErrorParam),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeExecutionResultBlockParam {
    pub content: Vec<CodeExecutionOutputBlockParam>,
    pub return_code: i32,
    pub stderr: String,
    pub stdout: String,
    #[serde(rename = "type")]
    pub type_: CodeExecutionResultBlockType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CodeExecutionResultBlockType {
    #[serde(rename = "code_execution_result")]
    CodeExecutionResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeExecutionOutputBlockParam {
    pub file_id: String,
    #[serde(rename = "type")]
    pub type_: CodeExecutionOutputBlockType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CodeExecutionOutputBlockType {
    #[serde(rename = "code_execution_output")]
    CodeExecutionOutput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedCodeExecutionResultBlockParam {
    pub content: Vec<CodeExecutionOutputBlockParam>,
    pub encrypted_stdout: String,
    pub return_code: i32,
    pub stderr: String,
    #[serde(rename = "type")]
    pub type_: EncryptedCodeExecutionResultBlockType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EncryptedCodeExecutionResultBlockType {
    #[serde(rename = "encrypted_code_execution_result")]
    EncryptedCodeExecutionResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeExecutionToolResultErrorParam {
    pub error_code: CodeExecutionToolResultErrorCode,
    #[serde(rename = "type")]
    pub type_: CodeExecutionToolResultErrorType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CodeExecutionToolResultErrorType {
    #[serde(rename = "code_execution_tool_result_error")]
    CodeExecutionToolResultError,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeExecutionToolResultErrorCode {
    InvalidToolInput,
    Unavailable,
    TooManyRequests,
    ExecutionTimeExceeded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BashCodeExecutionToolResultContentParam {
    Result(BashCodeExecutionResultBlockParam),
    Error(BashCodeExecutionToolResultErrorParam),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BashCodeExecutionResultBlockParam {
    pub content: Vec<BashCodeExecutionOutputBlockParam>,
    pub return_code: i32,
    pub stderr: String,
    pub stdout: String,
    #[serde(rename = "type")]
    pub type_: BashCodeExecutionResultBlockType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BashCodeExecutionResultBlockType {
    #[serde(rename = "bash_code_execution_result")]
    BashCodeExecutionResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BashCodeExecutionOutputBlockParam {
    pub file_id: String,
    #[serde(rename = "type")]
    pub type_: BashCodeExecutionOutputBlockType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BashCodeExecutionOutputBlockType {
    #[serde(rename = "bash_code_execution_output")]
    BashCodeExecutionOutput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BashCodeExecutionToolResultErrorParam {
    pub error_code: BashCodeExecutionToolResultErrorCode,
    #[serde(rename = "type")]
    pub type_: BashCodeExecutionToolResultErrorType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BashCodeExecutionToolResultErrorType {
    #[serde(rename = "bash_code_execution_tool_result_error")]
    BashCodeExecutionToolResultError,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BashCodeExecutionToolResultErrorCode {
    InvalidToolInput,
    Unavailable,
    TooManyRequests,
    ExecutionTimeExceeded,
    OutputFileTooLarge,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TextEditorCodeExecutionToolResultContentParam {
    ViewResult(TextEditorCodeExecutionViewResultBlockParam),
    CreateResult(TextEditorCodeExecutionCreateResultBlockParam),
    StrReplaceResult(TextEditorCodeExecutionStrReplaceResultBlockParam),
    Error(TextEditorCodeExecutionToolResultErrorParam),
}

#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextEditorCodeExecutionViewResultBlockParam {
    pub content: String,
    pub file_type: TextEditorFileType,
    #[serde(rename = "type")]
    pub type_: TextEditorViewResultType,
    pub num_lines: Option<u32>,
    pub start_line: Option<u32>,
    pub total_lines: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TextEditorViewResultType {
    #[serde(rename = "text_editor_code_execution_view_result")]
    TextEditorCodeExecutionViewResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextEditorFileType {
    Text,
    Image,
    Pdf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextEditorCodeExecutionCreateResultBlockParam {
    pub is_file_update: bool,
    #[serde(rename = "type")]
    pub type_: TextEditorCreateResultType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TextEditorCreateResultType {
    #[serde(rename = "text_editor_code_execution_create_result")]
    TextEditorCodeExecutionCreateResult,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextEditorCodeExecutionStrReplaceResultBlockParam {
    #[serde(rename = "type")]
    pub type_: TextEditorStrReplaceResultType,
    pub lines: Option<Vec<String>>,
    pub new_lines: Option<u32>,
    pub new_start: Option<u32>,
    pub old_lines: Option<u32>,
    pub old_start: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TextEditorStrReplaceResultType {
    #[serde(rename = "text_editor_code_execution_str_replace_result")]
    TextEditorCodeExecutionStrReplaceResult,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextEditorCodeExecutionToolResultErrorParam {
    pub error_code: TextEditorCodeExecutionToolResultErrorCode,
    #[serde(rename = "type")]
    pub type_: TextEditorToolResultErrorType,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TextEditorToolResultErrorType {
    #[serde(rename = "text_editor_code_execution_tool_result_error")]
    TextEditorCodeExecutionToolResultError,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextEditorCodeExecutionToolResultErrorCode {
    InvalidToolInput,
    Unavailable,
    TooManyRequests,
    ExecutionTimeExceeded,
    FileNotFound,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolSearchToolResultContentParam {
    Result(ToolSearchToolSearchResultBlockParam),
    Error(ToolSearchToolResultErrorParam),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSearchToolSearchResultBlockParam {
    pub tool_references: Vec<ToolReferenceBlockParam>,
    #[serde(rename = "type")]
    pub type_: ToolSearchResultType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ToolSearchResultType {
    #[serde(rename = "tool_search_tool_search_result")]
    ToolSearchToolSearchResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolReferenceBlockParam {
    pub tool_name: String,
    #[serde(rename = "type")]
    pub type_: ToolReferenceBlockType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControlEphemeral>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ToolReferenceBlockType {
    #[serde(rename = "tool_reference")]
    ToolReference,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSearchToolResultErrorParam {
    pub error_code: ToolSearchToolResultErrorCode,
    #[serde(rename = "type")]
    pub type_: ToolSearchToolResultErrorType,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ToolSearchToolResultErrorType {
    #[serde(rename = "tool_search_tool_result_error")]
    ToolSearchToolResultError,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolSearchToolResultErrorCode {
    InvalidToolInput,
    Unavailable,
    TooManyRequests,
    ExecutionTimeExceeded,
}

// ============================================================================
// Output Types (Response)
// ============================================================================

/// Message response from the API.
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    #[serde(rename = "type")]
    pub type_: MessageType,
    pub role: Role,
    pub content: Vec<ContentBlock>,
    pub model: String,
    pub stop_reason: Option<StopReason>,
    pub stop_sequence: Option<String>,
    pub usage: Usage,
    pub container: Option<Container>,
    pub stop_details: Option<StopDetails>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    #[serde(rename = "message")]
    Message,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text {
        text: String,
        citations: Option<Vec<TextCitation>>,
    },
    #[serde(rename = "thinking")]
    Thinking { signature: String, thinking: String },
    #[serde(rename = "redacted_thinking")]
    RedactedThinking { data: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
        caller: Option<ToolCaller>,
    },
    #[serde(rename = "server_tool_use")]
    ServerToolUse {
        id: String,
        name: ServerToolName,
        input: serde_json::Value,
        caller: Option<ToolCaller>,
    },
    #[serde(rename = "web_search_tool_result")]
    WebSearchToolResult {
        tool_use_id: String,
        content: WebSearchToolResultContent,
        caller: Option<ToolCaller>,
    },
    #[serde(rename = "web_fetch_tool_result")]
    WebFetchToolResult {
        tool_use_id: String,
        content: WebFetchToolResultContent,
        caller: Option<ToolCaller>,
    },
    #[serde(rename = "code_execution_tool_result")]
    CodeExecutionToolResult {
        tool_use_id: String,
        content: CodeExecutionToolResultContent,
    },
    #[serde(rename = "bash_code_execution_tool_result")]
    BashCodeExecutionToolResult {
        tool_use_id: String,
        content: BashCodeExecutionToolResultContent,
    },
    #[serde(rename = "text_editor_code_execution_tool_result")]
    TextEditorCodeExecutionToolResult {
        tool_use_id: String,
        content: TextEditorCodeExecutionToolResultContent,
    },
    #[serde(rename = "tool_search_tool_result")]
    ToolSearchToolResult {
        tool_use_id: String,
        content: ToolSearchToolResultContent,
    },
    #[serde(rename = "container_upload")]
    ContainerUpload { file_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TextCitation {
    #[serde(rename = "char_location")]
    CharLocation {
        cited_text: String,
        document_index: u32,
        document_title: String,
        start_char_index: u32,
        end_char_index: u32,
        file_id: String,
    },
    #[serde(rename = "page_location")]
    PageLocation {
        cited_text: String,
        document_index: u32,
        document_title: String,
        start_page_number: u32,
        end_page_number: u32,
        file_id: String,
    },
    #[serde(rename = "content_block_location")]
    ContentBlockLocation {
        cited_text: String,
        document_index: u32,
        document_title: String,
        start_block_index: u32,
        end_block_index: u32,
        file_id: String,
    },
    #[serde(rename = "web_search_result_location")]
    WebSearchResultLocation {
        cited_text: String,
        encrypted_index: String,
        title: String,
        url: String,
    },
    #[serde(rename = "search_result_location")]
    SearchResultLocation {
        cited_text: String,
        title: String,
        source: String,
        search_result_index: u32,
        start_block_index: u32,
        end_block_index: u32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WebSearchToolResultContent {
    Results(Vec<WebSearchResultBlock>),
    Error(WebSearchToolResultError),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearchResultBlock {
    pub encrypted_content: String,
    pub title: String,
    #[serde(rename = "type")]
    pub type_: WebSearchResultBlockType,
    pub url: String,
    pub page_age: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearchToolResultError {
    pub error_code: WebSearchToolResultErrorCode,
    #[serde(rename = "type")]
    pub type_: WebSearchToolResultErrorType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WebFetchToolResultContent {
    Result(WebFetchBlock),
    Error(WebFetchToolResultErrorBlock),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebFetchBlock {
    pub content: DocumentBlock,
    #[serde(rename = "type")]
    pub type_: WebFetchBlockType,
    pub url: String,
    pub retrieved_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentBlock {
    pub source: DocumentSource,
    pub title: String,
    #[serde(rename = "type")]
    pub type_: DocumentBlockType,
    pub citations: CitationsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DocumentSource {
    #[serde(rename = "base64")]
    Base64 {
        media_type: PdfMediaType,
        data: String,
    },
    #[serde(rename = "text")]
    Text {
        media_type: TextMediaType,
        data: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CitationsConfig {
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebFetchToolResultErrorBlock {
    pub error_code: WebFetchToolResultErrorCode,
    #[serde(rename = "type")]
    pub type_: WebFetchToolResultErrorType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CodeExecutionToolResultContent {
    Result(CodeExecutionResultBlock),
    EncryptedResult(EncryptedCodeExecutionResultBlock),
    Error(CodeExecutionToolResultError),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeExecutionResultBlock {
    pub content: Vec<CodeExecutionOutputBlock>,
    pub return_code: i32,
    pub stderr: String,
    pub stdout: String,
    #[serde(rename = "type")]
    pub type_: CodeExecutionResultBlockType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeExecutionOutputBlock {
    pub file_id: String,
    #[serde(rename = "type")]
    pub type_: CodeExecutionOutputBlockType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedCodeExecutionResultBlock {
    pub content: Vec<CodeExecutionOutputBlock>,
    pub encrypted_stdout: String,
    pub return_code: i32,
    pub stderr: String,
    #[serde(rename = "type")]
    pub type_: EncryptedCodeExecutionResultBlockType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeExecutionToolResultError {
    pub error_code: CodeExecutionToolResultErrorCode,
    #[serde(rename = "type")]
    pub type_: CodeExecutionToolResultErrorType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BashCodeExecutionToolResultContent {
    Result(BashCodeExecutionResultBlock),
    Error(BashCodeExecutionToolResultError),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BashCodeExecutionResultBlock {
    pub content: Vec<BashCodeExecutionOutputBlock>,
    pub return_code: i32,
    pub stderr: String,
    pub stdout: String,
    #[serde(rename = "type")]
    pub type_: BashCodeExecutionResultBlockType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BashCodeExecutionOutputBlock {
    pub file_id: String,
    #[serde(rename = "type")]
    pub type_: BashCodeExecutionOutputBlockType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BashCodeExecutionToolResultError {
    pub error_code: BashCodeExecutionToolResultErrorCode,
    #[serde(rename = "type")]
    pub type_: BashCodeExecutionToolResultErrorType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TextEditorCodeExecutionToolResultContent {
    ViewResult(TextEditorCodeExecutionViewResultBlock),
    CreateResult(TextEditorCodeExecutionCreateResultBlock),
    StrReplaceResult(TextEditorCodeExecutionStrReplaceResultBlock),
    Error(TextEditorCodeExecutionToolResultError),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextEditorCodeExecutionViewResultBlock {
    pub content: String,
    pub file_type: TextEditorFileType,
    pub num_lines: u32,
    pub start_line: u32,
    pub total_lines: u32,
    #[serde(rename = "type")]
    pub type_: TextEditorViewResultType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextEditorCodeExecutionCreateResultBlock {
    pub is_file_update: bool,
    #[serde(rename = "type")]
    pub type_: TextEditorCreateResultType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextEditorCodeExecutionStrReplaceResultBlock {
    pub lines: Vec<String>,
    pub new_lines: u32,
    pub new_start: u32,
    pub old_lines: u32,
    pub old_start: u32,
    #[serde(rename = "type")]
    pub type_: TextEditorStrReplaceResultType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextEditorCodeExecutionToolResultError {
    pub error_code: TextEditorCodeExecutionToolResultErrorCode,
    pub error_message: String,
    #[serde(rename = "type")]
    pub type_: TextEditorToolResultErrorType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolSearchToolResultContent {
    Result(ToolSearchToolSearchResultBlock),
    Error(ToolSearchToolResultError),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSearchToolSearchResultBlock {
    pub tool_references: Vec<ToolReferenceBlock>,
    #[serde(rename = "type")]
    pub type_: ToolSearchResultType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolReferenceBlock {
    pub tool_name: String,
    #[serde(rename = "type")]
    pub type_: ToolReferenceBlockType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSearchToolResultError {
    pub error_code: ToolSearchToolResultErrorCode,
    pub error_message: String,
    #[serde(rename = "type")]
    pub type_: ToolSearchToolResultErrorType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Container {
    pub id: String,
    pub expires_at: String,
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
#[serde(tag = "type")]
pub enum StopDetails {
    #[serde(rename = "refusal")]
    Refusal {
        category: Option<RefusalCategory>,
        explanation: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefusalCategory {
    Cyber,
    Bio,
    ReasoningExtraction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cache_creation_input_tokens: u32,
    pub cache_read_input_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_creation: Option<CacheCreation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_tokens_details: Option<OutputTokensDetails>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_tool_use: Option<ServerToolUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<ServiceTierResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inference_geo: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheCreation {
    pub ephemeral_5m_input_tokens: u32,
    pub ephemeral_1h_input_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputTokensDetails {
    pub thinking_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerToolUsage {
    pub web_fetch_requests: u32,
    pub web_search_requests: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceTierResponse {
    Standard,
    Priority,
    Batch,
}

// ============================================================================
// Request Module
// ============================================================================

pub mod request {
    use super::*;

    #[skip_serializing_none]
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MessagesRequest {
    pub model: String,
    pub messages: Vec<MessageParam>,
    pub max_tokens: u32,
    pub system: Option<SystemParam>,
    pub metadata: Option<Metadata>,
    pub stop_sequences: Option<Vec<String>>,
    pub stream: Option<bool>,
    pub temperature: Option<f32>,
    pub tool_choice: Option<ToolChoice>,
    pub tools: Option<Vec<Tool>>,
    pub top_k: Option<u32>,
    pub top_p: Option<f32>,
    pub thinking: Option<ThinkingConfig>,
    pub cache_control: Option<CacheControlEphemeral>,
    pub container: Option<String>,
    pub inference_geo: Option<String>,
    pub output_config: Option<OutputConfig>,
    pub service_tier: Option<ServiceTierRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SystemParam {
    Text(String),
    Blocks(Vec<TextBlockParam>),
}

#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub user_id: Option<String>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ToolChoice {
    #[serde(rename = "auto")]
    Auto {
        disable_parallel_tool_use: Option<bool>,
    },
    #[serde(rename = "any")]
    Any {
        disable_parallel_tool_use: Option<bool>,
    },
    #[serde(rename = "tool")]
    Tool {
        name: String,
        disable_parallel_tool_use: Option<bool>,
    },
    #[serde(rename = "none")]
    None,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Tool {
    #[serde(rename = "custom")]
    Custom {
        name: String,
        input_schema: ToolInputSchema,
        description: Option<String>,
        cache_control: Option<CacheControlEphemeral>,
        defer_loading: Option<bool>,
        eager_input_streaming: Option<bool>,
        input_examples: Option<Vec<serde_json::Value>>,
        strict: Option<bool>,
        allowed_callers: Option<Vec<AllowedCaller>>,
    },
    #[serde(rename = "bash_20250124")]
    Bash20250124 {
        name: BashToolName,
        cache_control: Option<CacheControlEphemeral>,
        defer_loading: Option<bool>,
        input_examples: Option<Vec<serde_json::Value>>,
        strict: Option<bool>,
        allowed_callers: Option<Vec<AllowedCaller>>,
    },
    #[serde(rename = "code_execution_20250522")]
    CodeExecution20250522 {
        name: CodeExecutionToolName,
        cache_control: Option<CacheControlEphemeral>,
        defer_loading: Option<bool>,
        strict: Option<bool>,
        allowed_callers: Option<Vec<AllowedCaller>>,
    },
    #[serde(rename = "code_execution_20250825")]
    CodeExecution20250825 {
        name: CodeExecutionToolName,
        cache_control: Option<CacheControlEphemeral>,
        defer_loading: Option<bool>,
        strict: Option<bool>,
        allowed_callers: Option<Vec<AllowedCaller>>,
    },
    #[serde(rename = "code_execution_20260120")]
    CodeExecution20260120 {
        name: CodeExecutionToolName,
        cache_control: Option<CacheControlEphemeral>,
        defer_loading: Option<bool>,
        strict: Option<bool>,
        allowed_callers: Option<Vec<AllowedCaller>>,
    },
    #[serde(rename = "memory_20250818")]
    Memory20250818 {
        name: MemoryToolName,
        cache_control: Option<CacheControlEphemeral>,
        defer_loading: Option<bool>,
        input_examples: Option<Vec<serde_json::Value>>,
        strict: Option<bool>,
        allowed_callers: Option<Vec<AllowedCaller>>,
    },
    #[serde(rename = "text_editor_20250124")]
    TextEditor20250124 {
        name: TextEditorToolName20250124,
        cache_control: Option<CacheControlEphemeral>,
        defer_loading: Option<bool>,
        input_examples: Option<Vec<serde_json::Value>>,
        strict: Option<bool>,
        allowed_callers: Option<Vec<AllowedCaller>>,
    },
    #[serde(rename = "text_editor_20250429")]
    TextEditor20250429 {
        name: TextEditorToolName20250429,
        cache_control: Option<CacheControlEphemeral>,
        defer_loading: Option<bool>,
        input_examples: Option<Vec<serde_json::Value>>,
        strict: Option<bool>,
        allowed_callers: Option<Vec<AllowedCaller>>,
    },
    #[serde(rename = "text_editor_20250728")]
    TextEditor20250728 {
        name: TextEditorToolName20250728,
        cache_control: Option<CacheControlEphemeral>,
        defer_loading: Option<bool>,
        input_examples: Option<Vec<serde_json::Value>>,
        max_characters: Option<u32>,
        strict: Option<bool>,
        allowed_callers: Option<Vec<AllowedCaller>>,
    },
    #[serde(rename = "web_search_20250305")]
    WebSearch20250305 {
        name: WebSearchToolName,
        cache_control: Option<CacheControlEphemeral>,
        defer_loading: Option<bool>,
        max_uses: Option<u32>,
        strict: Option<bool>,
        allowed_callers: Option<Vec<AllowedCaller>>,
        allowed_domains: Option<Vec<String>>,
        blocked_domains: Option<Vec<String>>,
        user_location: Option<UserLocation>,
    },
    #[serde(rename = "web_search_20260209")]
    WebSearch20260209 {
        name: WebSearchToolName,
        cache_control: Option<CacheControlEphemeral>,
        defer_loading: Option<bool>,
        max_uses: Option<u32>,
        strict: Option<bool>,
        allowed_callers: Option<Vec<AllowedCaller>>,
        allowed_domains: Option<Vec<String>>,
        blocked_domains: Option<Vec<String>>,
        user_location: Option<UserLocation>,
    },
    #[serde(rename = "web_fetch_20250910")]
    WebFetch20250910 {
        name: WebFetchToolName,
        cache_control: Option<CacheControlEphemeral>,
        defer_loading: Option<bool>,
        max_uses: Option<u32>,
        strict: Option<bool>,
        allowed_callers: Option<Vec<AllowedCaller>>,
        allowed_domains: Option<Vec<String>>,
        blocked_domains: Option<Vec<String>>,
        citations: Option<CitationsConfigParam>,
        max_content_tokens: Option<u32>,
    },
    #[serde(rename = "web_fetch_20260209")]
    WebFetch20260209 {
        name: WebFetchToolName,
        cache_control: Option<CacheControlEphemeral>,
        defer_loading: Option<bool>,
        max_uses: Option<u32>,
        strict: Option<bool>,
        allowed_callers: Option<Vec<AllowedCaller>>,
        allowed_domains: Option<Vec<String>>,
        blocked_domains: Option<Vec<String>>,
        citations: Option<CitationsConfigParam>,
        max_content_tokens: Option<u32>,
    },
    #[serde(rename = "web_fetch_20260309")]
    WebFetch20260309 {
        name: WebFetchToolName,
        cache_control: Option<CacheControlEphemeral>,
        defer_loading: Option<bool>,
        max_uses: Option<u32>,
        strict: Option<bool>,
        allowed_callers: Option<Vec<AllowedCaller>>,
        allowed_domains: Option<Vec<String>>,
        blocked_domains: Option<Vec<String>>,
        citations: Option<CitationsConfigParam>,
        max_content_tokens: Option<u32>,
        use_cache: Option<bool>,
    },
    #[serde(rename = "tool_search_tool_bm25_20251119")]
    ToolSearchBm2520251119 {
        name: ToolSearchBm25ToolName,
        cache_control: Option<CacheControlEphemeral>,
        defer_loading: Option<bool>,
        strict: Option<bool>,
        allowed_callers: Option<Vec<AllowedCaller>>,
    },
    #[serde(rename = "tool_search_tool_bm25")]
    ToolSearchBm25 {
        name: ToolSearchBm25ToolName,
        cache_control: Option<CacheControlEphemeral>,
        defer_loading: Option<bool>,
        strict: Option<bool>,
        allowed_callers: Option<Vec<AllowedCaller>>,
    },
    #[serde(rename = "tool_search_tool_regex_20251119")]
    ToolSearchRegex20251119 {
        name: ToolSearchRegexToolName,
        cache_control: Option<CacheControlEphemeral>,
        defer_loading: Option<bool>,
        strict: Option<bool>,
        allowed_callers: Option<Vec<AllowedCaller>>,
    },
    #[serde(rename = "tool_search_tool_regex")]
    ToolSearchRegex {
        name: ToolSearchRegexToolName,
        cache_control: Option<CacheControlEphemeral>,
        defer_loading: Option<bool>,
        strict: Option<bool>,
        allowed_callers: Option<Vec<AllowedCaller>>,
    },
}

#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInputSchema {
    #[serde(rename = "type")]
    pub type_: ToolInputSchemaType,
    pub properties: Option<serde_json::Value>,
    pub required: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ToolInputSchemaType {
    #[serde(rename = "object")]
    Object,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AllowedCaller {
    Direct,
    CodeExecution20250825,
    CodeExecution20260120,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BashToolName {
    #[serde(rename = "bash")]
    Bash,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CodeExecutionToolName {
    #[serde(rename = "code_execution")]
    CodeExecution,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemoryToolName {
    #[serde(rename = "memory")]
    Memory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TextEditorToolName20250124 {
    #[serde(rename = "str_replace_editor")]
    StrReplaceEditor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TextEditorToolName20250429 {
    #[serde(rename = "str_replace_based_edit_tool")]
    StrReplaceBasedEditTool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TextEditorToolName20250728 {
    #[serde(rename = "str_replace_based_edit_tool")]
    StrReplaceBasedEditTool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WebSearchToolName {
    #[serde(rename = "web_search")]
    WebSearch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WebFetchToolName {
    #[serde(rename = "web_fetch")]
    WebFetch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ToolSearchBm25ToolName {
    #[serde(rename = "tool_search_tool_bm25")]
    ToolSearchToolBm25,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ToolSearchRegexToolName {
    #[serde(rename = "tool_search_tool_regex")]
    ToolSearchToolRegex,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserLocation {
    #[serde(rename = "type")]
    pub type_: UserLocationType,
    pub city: Option<String>,
    pub country: Option<String>,
    pub region: Option<String>,
    pub timezone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UserLocationType {
    #[serde(rename = "approximate")]
    Approximate,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ThinkingConfig {
    #[serde(rename = "enabled")]
    Enabled {
        budget_tokens: u32,
        display: Option<ThinkingDisplay>,
    },
    #[serde(rename = "disabled")]
    Disabled,
    #[serde(rename = "adaptive")]
    Adaptive {
        display: Option<ThinkingDisplay>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThinkingDisplay {
    Summarized,
    Omitted,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    pub effort: Option<Effort>,
    pub format: Option<JsonOutputFormat>,
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
#[serde(tag = "type")]
pub enum JsonOutputFormat {
    #[serde(rename = "json_schema")]
    JsonSchema { schema: serde_json::Value },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceTierRequest {
    Auto,
    StandardOnly,
}

    #[skip_serializing_none]
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct CountTokensRequest {
        pub model: String,
        pub messages: Vec<MessageParam>,
        pub system: Option<SystemParam>,
        pub thinking: Option<ThinkingConfig>,
        pub tool_choice: Option<ToolChoice>,
        pub tools: Option<Vec<Tool>>,
        pub cache_control: Option<CacheControlEphemeral>,
    }
}

// ============================================================================
// Response Module
// ============================================================================

pub mod response {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MessageTokensCount {
        pub input_tokens: u32,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(tag = "type")]
    pub enum MessageStreamEvent {
    #[serde(rename = "message_start")]
    MessageStart { message: Message },
    #[serde(rename = "ping")]
    Ping,
    #[serde(rename = "content_block_start")]
    ContentBlockStart {
        index: u32,
        content_block: ContentBlock,
    },
    #[serde(rename = "content_block_delta")]
    ContentBlockDelta {
        index: u32,
        delta: ContentBlockDelta,
    },
    #[serde(rename = "content_block_stop")]
    ContentBlockStop { index: u32 },
    #[serde(rename = "message_delta")]
    MessageDelta { delta: MessageDelta, usage: Usage },
    #[serde(rename = "message_stop")]
    MessageStop,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlockDelta {
    #[serde(rename = "text_delta")]
    TextDelta { text: String },
    #[serde(rename = "input_json_delta")]
    InputJsonDelta { partial_json: String },
    #[serde(rename = "thinking_delta")]
    ThinkingDelta { thinking: String },
    #[serde(rename = "signature_delta")]
    SignatureDelta { signature: String },
}

#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageDelta {
    pub stop_reason: Option<StopReason>,
    pub stop_sequence: Option<String>,
    pub stop_details: Option<StopDetails>,
    pub container: Option<Container>,
}
}

// ============================================================================
// API Functions
// ============================================================================

pub async fn generate(
    client: reqwest::Client,
    mut request: request::MessagesRequest,
) -> Result<Message, super::Error> {
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
