// Types matching the OpenAPI schema

export interface TraceMetadata {
    id: string;
    name: string;
    created: string;
    modified: string;
}

export type TraceStoreEvent = {
    type: "added";
    trace: TraceMetadata;
};

export type TraceEvent =
    | { type: "message_start"; position: number; role: Role }
    | { type: "thinking_delta"; delta: string }
    | { type: "content_delta"; delta: string }
    | { type: "message_end"; position: number }
    | { type: "error"; message: string };

export type Role = "system" | "assistant" | "user";

export interface Message {
    role: Role;
    content: string;
    thinking?: string;
    position: number; // Track position for building from deltas
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
    messages: ChatMessage[];
}
