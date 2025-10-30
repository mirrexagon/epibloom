use std::sync::Mutex;

use rocket::{get, response::status, serde::json::Json, State};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, OpenApi, ToSchema};

use super::ApiErrorResponse;
use crate::llm_session;

// TODO:
// GET /: Get list of LLM sessions, metadata only.
// GET /<LLM session ID>: Get the sessions and stay connected for any streaming updates for incoming messages/text/etc. On disconnection, subscription is cancelled, but LLM session still runs in the background. Can support multiple parallel requesters and gives the updates to each one.

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct GetLlmSessionsResponse {
    llm_sessions: Vec<llm_session::LlmSessionMetadata>,
}

/// Gets a list of all stored games.
#[get("/")]
pub fn get_llm_sessions() -> Result<Json<GetLlmSessionsResponse>, ApiErrorResponse> {
    let response = GetLlmSessionsResponse {
        llm_sessions: Vec::new(),
    };
    Ok(Json(response))
}
