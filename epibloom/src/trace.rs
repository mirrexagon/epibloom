//! An LLM conversation, with message updates and generation chunks streamed to
//! clients.

mod conversation;
mod persistence;
mod session;
mod store;

pub use conversation::{Message, Role, Trace};
pub use session::TraceEvent;
pub use store::{TraceStore, TraceStoreEvent};

// TODO: For Claude, just build up the response in a State object and have the
// UI interpret and display it. That way it can include things like usage stats.

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("trace not found")]
    TraceNotFound,

    #[error("Backend error: {0}")]
    Backend(String),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
