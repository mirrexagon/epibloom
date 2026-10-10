// Types matching the OpenAPI schema and backend data structures

import { Operation } from "fast-json-patch";

// ---- State Objects API ----------

export type StateEvent =
    | {
          type: "value";
          object: string;
          value: unknown;
      }
    | {
          type: "patch";
          object: string;
          patch: Operation[];
      };

export interface StateObjectData {
    name: string;
    value: unknown;
}

// ---- Anthropic LLM Traces ----------

export interface AnthropicTraceStateObject {
    start_time: string; // RFC3339 datetime
    end_time: string | null; // RFC3339 datetime or null if still running
    request: AnthropicRequest;
    response: AnthropicResponse | null;
    error: string | null;
}

export interface AnthropicRequest {
    model: string;
    max_tokens: number;
    system?: string; // Optional system prompt
    messages: AnthropicMessage[];
}

export interface AnthropicResponse {
    id: string;
    type: string;
    role: string;
    content: AnthropicContentBlock[];
    model: string;
    stop_reason: string;
    usage: AnthropicUsage;
}

export interface AnthropicMessage {
    role: "user" | "assistant" | "system";
    content: string | AnthropicContentBlock[];
    id?: string; // Present on assistant messages
    model?: string; // Present on assistant messages
    stop_reason?: string; // Present on assistant messages
    usage?: AnthropicUsage; // Present on assistant messages
}

export type AnthropicContentBlock =
    | {
          type: "text";
          text: string;
      }
    | {
          type: "thinking";
          thinking: string;
      };

export interface AnthropicUsage {
    input_tokens: number;
    output_tokens: number;
    cache_creation_input_tokens?: number;
    cache_read_input_tokens?: number;
    cache_creation?: {
        ephemeral_5m_input_tokens: number;
        ephemeral_1h_input_tokens: number;
    };
    service_tier?: string;
    inference_geo?: string;
}

// ---- Scripts ----------

/** An entry in a `script-status:<context>` state object's array. */
export interface ScriptStatus {
    script_path: string;
    request_time: string; // RFC3339 datetime
    start_time: string | null;
    end_time: string | null; // Set when the script has finished (successfully or not)
    input: unknown;
    result: unknown;
    error: string | null;
}

// ---- Chat (see epibloom/scripts/chat.lua) ----------
// Note: all indices are 1-based, matching the Lua side.

/** Stored in the `chat:<id>:persist` state object. */
export interface Chat {
    id: string;
    name: string;
    system: string;
    model: string;
    max_tokens: number;
    messages: ChatMessageSlot[];
    created: string;
    modified: string;
}

/** Stored in the (non-persisted) `chat:<id>` state object. May be null if not yet initialised. */
export interface ChatStatus {
    generating: boolean;
    cancel_requested: boolean;
    message_index?: number;
    alternate_index?: number;
    trace?: string;
}

export interface ChatMessageSlot {
    alternates: ChatMessage[];
    /** 1-based index into `alternates`. */
    selected: number;
}

export type ChatRole = "user" | "assistant";

export interface ChatMessage {
    role: ChatRole;
    /** List of text blocks, each of which is a list of chunks to be concatenated. */
    content: string[][];
    note: string;
    /** Name of the trace state object for the generation that produced this message. */
    trace?: string;
    error?: string;
    created: string;
    modified: string;
}
