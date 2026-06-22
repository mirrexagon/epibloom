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
