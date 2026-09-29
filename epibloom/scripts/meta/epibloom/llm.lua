--- @meta

--- LLM generation.
---
--- Request and response tables mirror the Anthropic Messages API
--- (https://docs.claude.com/en/api/messages), as defined in
--- `epibloom/src/backend/anthropic/`.
--- @class EpibloomLlmModule
local llm = {}

--- List all models available from the Anthropic API.
--- @return anthropic.ModelInfo[]? models # The list of models, or `nil` on error.
--- @return string? error # Error message if the request failed.
--- @nodiscard
function llm.anthropic_list_models() end

--- Start a non-streaming generation. Generation happens in the background;
--- use the returned handle to wait for the response or cancel it.
---
--- Raises an error if the request is invalid or generation can't be started.
--- @param request anthropic.MessagesRequest
--- @return AnthropicGenerateHandle
--- @nodiscard
function llm.anthropic_generate(request) end

--- Start a streaming generation. Generation happens in the background;
--- use the returned handle to iterate over stream events, wait for the full
--- response, or cancel it.
---
--- Raises an error if the request is invalid or generation can't be started.
--- @param request anthropic.MessagesRequest # `stream` is ignored and always set to `true`.
--- @return AnthropicGenerateStreamingHandle
--- @nodiscard
function llm.anthropic_generate_streaming(request) end

--- Count the number of input tokens a request would use.
--- @param request anthropic.CountTokensRequest
--- @return anthropic.MessageTokensCount? count # The token count, or `nil` on error.
--- @return string? error # Error message if the request failed.
--- @nodiscard
function llm.anthropic_count_tokens(request) end

-- ============================================================================
-- Handles
-- ============================================================================

--- Handle to a non-streaming generation started with `llm.anthropic_generate()`.
--- @class AnthropicGenerateHandle
--- @field name string # Name of the state object holding the trace for this generation.
local AnthropicGenerateHandle = {}

--- Wait for the generation to complete and return the response. Can be
--- called multiple times, subsequent calls return the same result.
--- @return anthropic.Message? response # The response, or `nil` on error.
--- @return string? error # Error message if generation failed.
--- @nodiscard
function AnthropicGenerateHandle:get_response() end

--- Cancel the generation.
function AnthropicGenerateHandle:cancel() end

--- Handle to a streaming generation started with `llm.anthropic_generate_streaming()`.
---
--- Calling the handle waits for and returns the next stream event:
--- - `true, event` for a stream event.
--- - `false, error_message` if an error occurred.
--- - `nil` when generation is complete (or after an error).
---
--- This means it can be used directly in a generic `for` loop:
--- ```lua
--- for ok, event in handle do
---     if not ok then error(event) end
---     -- ...
--- end
--- ```
--- @class AnthropicGenerateStreamingHandle
--- @field name string # Name of the state object holding the trace for this generation.
--- @overload fun(): (ok: boolean?, event: anthropic.MessageStreamEvent|string?)
local AnthropicGenerateStreamingHandle = {}

--- Drain any remaining stream events and return the accumulated response.
--- @return anthropic.Message? response # The response accumulated so far (possibly partial if an error occurred), or `nil` if nothing was received.
--- @return string? error # Error message if generation failed.
--- @nodiscard
function AnthropicGenerateStreamingHandle:get_response() end

--- Cancel the generation.
function AnthropicGenerateStreamingHandle:cancel() end

-- ============================================================================
-- Common types
-- ============================================================================

--- @alias anthropic.Role "user" | "assistant" | "system"

--- @class anthropic.CacheControl
--- @field type "ephemeral"
--- @field ttl? "5m" | "1h"

--- @alias anthropic.ServerToolName
--- | "web_search"
--- | "web_fetch"
--- | "code_execution"
--- | "bash_code_execution"
--- | "text_editor_code_execution"
--- | "tool_search_tool_regex"
--- | "tool_search_tool_bm25"

--- @alias anthropic.ToolCaller
--- | { type: "direct" }
--- | { type: "code_execution_20250825", tool_id: string }
--- | { type: "code_execution_20260120", tool_id: string }

-- ============================================================================
-- Request types
-- ============================================================================

--- @class anthropic.MessagesRequest
--- @field model string # eg. `"claude-sonnet-4-5"`.
--- @field messages anthropic.MessageParam[]
--- @field max_tokens integer
--- @field system? string | anthropic.TextBlockParam[]
--- @field metadata? { user_id?: string }
--- @field stop_sequences? string[]
--- @field stream? boolean # Ignored; set automatically depending on the generate function used.
--- @field temperature? number
--- @field tool_choice? anthropic.ToolChoice
--- @field tools? anthropic.Tool[]
--- @field top_k? integer
--- @field top_p? number
--- @field thinking? anthropic.ThinkingConfig
--- @field cache_control? anthropic.CacheControl
--- @field container? string
--- @field inference_geo? string
--- @field output_config? anthropic.OutputConfig
--- @field service_tier? "auto" | "standard_only"

--- @class anthropic.CountTokensRequest
--- @field model string
--- @field messages anthropic.MessageParam[]
--- @field system? string | anthropic.TextBlockParam[]
--- @field thinking? anthropic.ThinkingConfig
--- @field tool_choice? anthropic.ToolChoice
--- @field tools? anthropic.Tool[]
--- @field cache_control? anthropic.CacheControl

--- @class anthropic.MessageParam
--- @field role anthropic.Role
--- @field content string | anthropic.ContentBlockParam[]

--- @alias anthropic.ContentBlockParam
--- | anthropic.TextBlockParam
--- | anthropic.ImageBlockParam
--- | anthropic.DocumentBlockParam
--- | anthropic.SearchResultBlockParam
--- | anthropic.ThinkingBlockParam
--- | anthropic.RedactedThinkingBlockParam
--- | anthropic.ToolUseBlockParam
--- | anthropic.ToolResultBlockParam
--- | anthropic.ServerToolUseBlockParam
--- | anthropic.ServerToolResultBlockParam
--- | anthropic.ContainerUploadBlockParam
--- | anthropic.MidConvSystemBlockParam

--- @class anthropic.TextBlockParam
--- @field type "text"
--- @field text string
--- @field cache_control? anthropic.CacheControl
--- @field citations? anthropic.TextCitationParam[]

--- @class anthropic.ImageBlockParam
--- @field type "image"
--- @field source anthropic.ImageSourceParam
--- @field cache_control? anthropic.CacheControl

--- @alias anthropic.ImageSourceParam
--- | { type: "base64", media_type: "image/jpeg" | "image/png" | "image/gif" | "image/webp", data: string }
--- | { type: "url", url: string }

--- @class anthropic.DocumentBlockParam
--- @field type "document"
--- @field source anthropic.DocumentSourceParam
--- @field cache_control? anthropic.CacheControl
--- @field citations? anthropic.CitationsConfigParam
--- @field context? string
--- @field title? string

--- @alias anthropic.DocumentSourceParam
--- | { type: "base64", media_type: "application/pdf", data: string }
--- | { type: "text", media_type: "text/plain", data: string }
--- | { type: "content", content: string | (anthropic.TextBlockParam | anthropic.ImageBlockParam)[] }
--- | { type: "url", url: string }

--- @class anthropic.CitationsConfigParam
--- @field enabled? boolean

--- @class anthropic.SearchResultBlockParam
--- @field type "search_result"
--- @field content anthropic.TextBlockParam[]
--- @field source string
--- @field title string
--- @field cache_control? anthropic.CacheControl
--- @field citations? anthropic.CitationsConfigParam

--- @class anthropic.ThinkingBlockParam
--- @field type "thinking"
--- @field thinking string
--- @field signature string

--- @class anthropic.RedactedThinkingBlockParam
--- @field type "redacted_thinking"
--- @field data string

--- @class anthropic.ToolUseBlockParam
--- @field type "tool_use"
--- @field id string
--- @field name string
--- @field input any
--- @field cache_control? anthropic.CacheControl
--- @field caller? anthropic.ToolCaller

--- @class anthropic.ToolResultBlockParam
--- @field type "tool_result"
--- @field tool_use_id string
--- @field content? string | anthropic.ToolResultContentBlockParam[]
--- @field is_error? boolean
--- @field cache_control? anthropic.CacheControl

--- @alias anthropic.ToolResultContentBlockParam
--- | anthropic.TextBlockParam
--- | anthropic.ImageBlockParam
--- | anthropic.SearchResultBlockParam
--- | anthropic.DocumentBlockParam
--- | { type: "tool_reference", tool_name: string, cache_control?: anthropic.CacheControl }

--- @class anthropic.ServerToolUseBlockParam
--- @field type "server_tool_use"
--- @field id string
--- @field name anthropic.ServerToolName
--- @field input any
--- @field cache_control? anthropic.CacheControl
--- @field caller? anthropic.ToolCaller

--- Results of server-side tools, usually copied back from a previous response.
--- `content` has a tool-specific shape; see `epibloom/src/backend/anthropic/messages.rs`.
--- @class anthropic.ServerToolResultBlockParam
--- @field type "web_search_tool_result" | "web_fetch_tool_result" | "code_execution_tool_result" | "bash_code_execution_tool_result" | "text_editor_code_execution_tool_result" | "tool_search_tool_result"
--- @field tool_use_id string
--- @field content table
--- @field cache_control? anthropic.CacheControl
--- @field caller? anthropic.ToolCaller # Only for `web_search_tool_result` and `web_fetch_tool_result`.

--- @class anthropic.ContainerUploadBlockParam
--- @field type "container_upload"
--- @field file_id string
--- @field cache_control? anthropic.CacheControl

--- @class anthropic.MidConvSystemBlockParam
--- @field type "mid_conv_system"
--- @field content anthropic.TextBlockParam[]
--- @field cache_control? anthropic.CacheControl

--- @alias anthropic.TextCitationParam
--- | { type: "char_location", cited_text: string, document_index: integer, document_title: string, start_char_index: integer, end_char_index: integer }
--- | { type: "page_location", cited_text: string, document_index: integer, document_title: string, start_page_number: integer, end_page_number: integer }
--- | { type: "content_block_location", cited_text: string, document_index: integer, document_title: string, start_block_index: integer, end_block_index: integer }
--- | { type: "web_search_result_location", cited_text: string, encrypted_index: string, title: string, url: string }
--- | { type: "search_result_location", cited_text: string, title: string, source: string, search_result_index: integer, start_block_index: integer, end_block_index: integer }

--- @alias anthropic.ToolChoice
--- | { type: "auto", disable_parallel_tool_use?: boolean }
--- | { type: "any", disable_parallel_tool_use?: boolean }
--- | { type: "tool", name: string, disable_parallel_tool_use?: boolean }
--- | { type: "none" }

--- @alias anthropic.Tool anthropic.CustomTool | anthropic.ServerTool

--- @alias anthropic.AllowedCaller "direct" | "code_execution_20250825" | "code_execution_20260120"

--- A client-side tool.
--- @class anthropic.CustomTool
--- @field type "custom"
--- @field name string
--- @field input_schema anthropic.ToolInputSchema
--- @field description? string
--- @field cache_control? anthropic.CacheControl
--- @field defer_loading? boolean
--- @field eager_input_streaming? boolean
--- @field input_examples? any[]
--- @field strict? boolean
--- @field allowed_callers? anthropic.AllowedCaller[]

--- @class anthropic.ToolInputSchema
--- @field type "object"
--- @field properties? table<string, any> # JSON Schema properties.
--- @field required? string[]

--- An Anthropic-defined tool. Which fields are valid depends on `type`; see
--- `epibloom/src/backend/anthropic/messages.rs`.
--- @class anthropic.ServerTool
--- @field type "bash_20250124" | "code_execution_20250522" | "code_execution_20250825" | "code_execution_20260120" | "memory_20250818" | "text_editor_20250124" | "text_editor_20250429" | "text_editor_20250728" | "web_search_20250305" | "web_search_20260209" | "web_fetch_20250910" | "web_fetch_20260209" | "web_fetch_20260309" | "tool_search_tool_bm25_20251119" | "tool_search_tool_bm25" | "tool_search_tool_regex_20251119" | "tool_search_tool_regex"
--- @field name "bash" | "code_execution" | "memory" | "str_replace_editor" | "str_replace_based_edit_tool" | "web_search" | "web_fetch" | "tool_search_tool_bm25" | "tool_search_tool_regex"
--- @field cache_control? anthropic.CacheControl
--- @field defer_loading? boolean
--- @field input_examples? any[]
--- @field strict? boolean
--- @field allowed_callers? anthropic.AllowedCaller[]
--- @field max_characters? integer # text_editor_20250728 only.
--- @field max_uses? integer # web_search and web_fetch only.
--- @field allowed_domains? string[] # web_search and web_fetch only.
--- @field blocked_domains? string[] # web_search and web_fetch only.
--- @field user_location? anthropic.UserLocation # web_search only.
--- @field citations? anthropic.CitationsConfigParam # web_fetch only.
--- @field max_content_tokens? integer # web_fetch only.
--- @field use_cache? boolean # web_fetch_20260309 only.

--- @class anthropic.UserLocation
--- @field type "approximate"
--- @field city? string
--- @field country? string
--- @field region? string
--- @field timezone? string

--- @alias anthropic.ThinkingConfig
--- | { type: "disabled" }
--- | { type: "enabled", budget_tokens: integer, display?: "summarized" | "omitted" }
--- | { type: "adaptive", display?: "summarized" | "omitted" }

--- @class anthropic.OutputConfig
--- @field effort? "low" | "medium" | "high" | "xhigh" | "max"
--- @field format? { type: "json_schema", schema: table }

-- ============================================================================
-- Response types
-- ============================================================================

--- A message returned by the API.
--- @class anthropic.Message
--- @field id string
--- @field type "message"
--- @field role anthropic.Role
--- @field content anthropic.ContentBlock[]
--- @field model string
--- @field stop_reason? anthropic.StopReason
--- @field stop_sequence? string
--- @field usage anthropic.Usage
--- @field container? anthropic.Container
--- @field stop_details? anthropic.StopDetails

--- @alias anthropic.StopReason "end_turn" | "max_tokens" | "stop_sequence" | "tool_use" | "pause_turn" | "refusal"

--- @class anthropic.StopDetails
--- @field type "refusal"
--- @field category? "cyber" | "bio" | "reasoning_extraction"
--- @field explanation? string

--- @class anthropic.Container
--- @field id string
--- @field expires_at string

--- @class anthropic.Usage
--- @field input_tokens integer
--- @field output_tokens integer
--- @field cache_creation_input_tokens integer
--- @field cache_read_input_tokens integer
--- @field cache_creation? { ephemeral_5m_input_tokens: integer, ephemeral_1h_input_tokens: integer }
--- @field output_tokens_details? { thinking_tokens: integer }
--- @field server_tool_use? { web_fetch_requests: integer, web_search_requests: integer }
--- @field service_tier? "standard" | "priority" | "batch"
--- @field inference_geo? string

--- @alias anthropic.ContentBlock
--- | anthropic.TextBlock
--- | anthropic.ThinkingBlock
--- | anthropic.RedactedThinkingBlock
--- | anthropic.ToolUseBlock
--- | anthropic.ServerToolUseBlock
--- | anthropic.ServerToolResultBlock
--- | anthropic.ContainerUploadBlock

--- @class anthropic.TextBlock
--- @field type "text"
--- @field text string
--- @field citations? anthropic.TextCitation[]

--- @class anthropic.ThinkingBlock
--- @field type "thinking"
--- @field thinking string
--- @field signature string

--- @class anthropic.RedactedThinkingBlock
--- @field type "redacted_thinking"
--- @field data string

--- @class anthropic.ToolUseBlock
--- @field type "tool_use"
--- @field id string
--- @field name string
--- @field input any # While streaming, this may be a string of partial JSON until it can be parsed.
--- @field caller? anthropic.ToolCaller

--- @class anthropic.ServerToolUseBlock
--- @field type "server_tool_use"
--- @field id string
--- @field name anthropic.ServerToolName
--- @field input any # While streaming, this may be a string of partial JSON until it can be parsed.
--- @field caller? anthropic.ToolCaller

--- Result of a server-side tool. `content` has a tool-specific shape; see
--- `epibloom/src/backend/anthropic/messages.rs`.
--- @class anthropic.ServerToolResultBlock
--- @field type "web_search_tool_result" | "web_fetch_tool_result" | "code_execution_tool_result" | "bash_code_execution_tool_result" | "text_editor_code_execution_tool_result" | "tool_search_tool_result"
--- @field tool_use_id string
--- @field content table
--- @field caller? anthropic.ToolCaller # Only for `web_search_tool_result` and `web_fetch_tool_result`.

--- @class anthropic.ContainerUploadBlock
--- @field type "container_upload"
--- @field file_id string

--- @alias anthropic.TextCitation
--- | { type: "char_location", cited_text: string, document_index: integer, document_title: string, start_char_index: integer, end_char_index: integer, file_id: string }
--- | { type: "page_location", cited_text: string, document_index: integer, document_title: string, start_page_number: integer, end_page_number: integer, file_id: string }
--- | { type: "content_block_location", cited_text: string, document_index: integer, document_title: string, start_block_index: integer, end_block_index: integer, file_id: string }
--- | { type: "web_search_result_location", cited_text: string, encrypted_index: string, title: string, url: string }
--- | { type: "search_result_location", cited_text: string, title: string, source: string, search_result_index: integer, start_block_index: integer, end_block_index: integer }

--- @class anthropic.MessageTokensCount
--- @field input_tokens integer

-- ============================================================================
-- Streaming event types
-- ============================================================================

--- @alias anthropic.MessageStreamEvent
--- | { type: "message_start", message: anthropic.Message }
--- | { type: "ping" }
--- | { type: "content_block_start", index: integer, content_block: anthropic.ContentBlock }
--- | { type: "content_block_delta", index: integer, delta: anthropic.ContentBlockDelta }
--- | { type: "content_block_stop", index: integer }
--- | { type: "message_delta", delta: anthropic.MessageDelta, usage: anthropic.Usage }
--- | { type: "message_stop" }

--- @alias anthropic.ContentBlockDelta
--- | { type: "text_delta", text: string }
--- | { type: "input_json_delta", partial_json: string }
--- | { type: "thinking_delta", thinking: string }
--- | { type: "signature_delta", signature: string }

--- @class anthropic.MessageDelta
--- @field stop_reason? anthropic.StopReason
--- @field stop_sequence? string
--- @field stop_details? anthropic.StopDetails
--- @field container? anthropic.Container

-- ============================================================================
-- Model info types
-- ============================================================================

--- @class anthropic.ModelInfo
--- @field id string
--- @field display_name string
--- @field created_at string # RFC 3339 timestamp.
--- @field capabilities anthropic.ModelCapabilities
--- @field max_input_tokens integer
--- @field max_tokens integer

--- @alias anthropic.CapabilitySupport { supported: boolean }

--- @class anthropic.ModelCapabilities
--- @field batch anthropic.CapabilitySupport
--- @field citations anthropic.CapabilitySupport
--- @field code_execution anthropic.CapabilitySupport
--- @field context_management { supported: boolean, clear_thinking_20251015: anthropic.CapabilitySupport, clear_tool_uses_20250919: anthropic.CapabilitySupport, compact_20260112: anthropic.CapabilitySupport }
--- @field effort { supported: boolean, low: anthropic.CapabilitySupport, medium: anthropic.CapabilitySupport, high: anthropic.CapabilitySupport, xhigh: anthropic.CapabilitySupport, max: anthropic.CapabilitySupport }
--- @field image_input anthropic.CapabilitySupport
--- @field pdf_input anthropic.CapabilitySupport
--- @field structured_outputs anthropic.CapabilitySupport
--- @field thinking { supported: boolean, types: { enabled: anthropic.CapabilitySupport, adaptive: anthropic.CapabilitySupport } }

return llm
