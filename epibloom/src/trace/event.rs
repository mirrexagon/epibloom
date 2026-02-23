use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::conversation::{Role, TraceMetadata};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TraceStoreEvent {
    Added { trace: TraceMetadata },
}

// TODO: Use JSON Patch of an entire trace instead of custom events?
// Implement traces in terms of StateObjects and have backends and backened
// reuqests exposed directly to Lua where scripts can feed chunks into state
// objects that then get transmitted to the clients?
// Or have traces in Rust still, exposed to Lua in the same way, but use the state objects to transmit to clients.
//
// Maybe a bit crazy. Streaming message generation would mean sending the whole message string across with each update
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TraceEvent {
    MessageStart { position: usize, role: Role },
    ThinkingDelta { delta: String },
    ContentDelta { delta: String },
    MessageEnd { position: usize },
    Error { message: String },
}
