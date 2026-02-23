//! An LLM conversation, with message updates and generation chunks streamed to
//! clients.

mod conversation;
mod event;
mod persistence;
mod session;
mod store;

pub use conversation::{Message, Role, Trace};
pub use event::{TraceEvent, TraceStoreEvent};
pub use store::TraceStore;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("trace not found")]
    TraceNotFound,

    #[error("trace has no messages")]
    TraceHasNoMessages,

    #[error("message not found")]
    MessageNotFound,

    #[error("can't modify trace while generating")]
    CantModifyTraceWhileGenerating,

    #[error("thinking was supplied for a non-assistant message")]
    ThinkingSuppliedForNonAssistantMessage,

    #[error("Backend error: {0}")]
    Backend(String),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
