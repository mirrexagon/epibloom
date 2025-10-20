//! Straight generating from the LLM, mostly for testing.


use rocket::{serde::json::Json, State};
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};

use crate::backends::{
    anthropic::{self, AnthropicApi},
    featherless::{self, FeatherlessApi},
    Backend, Chat,
};

use super::ApiErrorResponse;

impl From<anthropic::Error> for ApiErrorResponse {
    fn from(value: anthropic::Error) -> Self {
        ApiErrorResponse::BadGateway(value.to_string())
    }
}

impl From<featherless::Error> for ApiErrorResponse {
    fn from(value: featherless::Error) -> Self {
        ApiErrorResponse::BadGateway(value.to_string())
    }
}

#[derive(OpenApi)]
#[openapi(paths(generate_claude, generate_featherless))]
pub struct BackendProxyApiDoc;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct GenerateResponse {
    message: String,
}

/// Generates an assistant response on the end of a chat using Claude.
#[utoipa::path(
    responses(
        (status = 200, description = "Message generated succesfully.", body = GenerateResponse),
        (status = 502, description = "LLM backend error.", body = String)
    )
)]
#[post("/generate-claude", data = "<chat>")]
pub async fn generate_claude(
    chat: Json<Chat>,
    anthropic_api: &State<AnthropicApi>,
) -> Result<Json<GenerateResponse>, ApiErrorResponse> {
    // TODO: Let client decide this, probably by choosing stored config values
    let anthropic_config = anthropic::Config::default();

    // TODO for continue: Anthropic API returned error 'invalid_request_error': messages: final assistant content cannot end with trailing whitespace
    let new_message = anthropic_api.generate(&anthropic_config, &chat).await?;

    Ok(Json(GenerateResponse {
        message: new_message,
    }))
}

/// Generates an assistant response on the end of a chat using Featherless.
#[utoipa::path(
    responses(
        (status = 200, description = "Message generated succesfully.", body = GenerateResponse),
        (status = 502, description = "LLM backend error.", body = String)
    )
)]
#[post("/generate-featherless", data = "<chat>")]
pub async fn generate_featherless(
    chat: Json<Chat>,
    featherless_api: &State<FeatherlessApi>,
) -> Result<Json<GenerateResponse>, ApiErrorResponse> {
    // TODO: Let client decide this, probably by choosing stored config values
    let featherless_config = featherless::Config::default();

    let new_message = featherless_api.generate(&featherless_config, &chat).await?;

    Ok(Json(GenerateResponse {
        message: new_message,
    }))
}
