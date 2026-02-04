// Types matching the OpenAPI schema

export interface TraceMetadata {
  id: string;
  name: string;
  created: string;
  modified: string;
}

export type TraceStoreEvent = {
  type: 'added';
  trace: TraceMetadata;
};

export type TraceEvent =
  | { type: 'user_message_start'; position: number }
  | { type: 'user_message_delta'; delta: string }
  | { type: 'user_message_end'; position: number }
  | { type: 'assistant_message_start'; position: number }
  | { type: 'assistant_thinking_delta'; delta: string }
  | { type: 'assistant_message_delta'; delta: string }
  | { type: 'assistant_message_end'; position: number }
  | { type: 'error'; message: string };

export type Role = 'system' | 'assistant' | 'user';

export interface Message {
  role: Role;
  text: string;
  thinking?: string;
  position: number; // Track position for building from deltas
}
