// Types matching the OpenAPI schema

export type Role = "system" | "assistant" | "user";

// ---- LLM trace (stored as state objects named "llm-trace:<uuid>") ----------

export interface LlmTraceMessage {
    role: Role;
    /** Content chunks — join to get the full text. */
    content: string[];
    /** Thinking chunks — join to get the full text. Present for assistant turns. */
    thinking?: string[];
    /** True while the assistant response is still being streamed. */
    generating?: boolean;
    /** Error message if generation failed. */
    error?: string;
}

export interface LlmTrace {
    id: string;
    name: string;
    created: string;
    messages: LlmTraceMessage[];
}

/** Entry in the "llm-trace-index" state object array. */
export interface LlmTraceIndexEntry {
    id: string;
    name: string;
    created: string;
}

// State objects types
export type StateEvent =
    | {
          type: "value";
          object: string;
          value: unknown;
      }
    | {
          type: "patch";
          object: string;
          patch: Array<{
              op: string;
              path: string;
              value?: unknown;
          }>;
      };

export interface StateObjectData {
    name: string;
    value: unknown;
}

// Chat types
export interface ChatMessage {
    id: string;
    parent_id: string | null;
    role: Role;
    // Content is stored as a list of string chunks for efficient streaming via
    // JSON Patch array-append operations. Join them to get the full text.
    content: string[];
    // Thinking text as a list of string chunks, if present. Join them to get the full text.
    thinking?: string[];
    trace_id?: string;
    // If an error occurred generating the message, this will have the error message.
    error?: string;
}

export interface Chat {
    id: string;
    messages: Record<string, ChatMessage>;
}
