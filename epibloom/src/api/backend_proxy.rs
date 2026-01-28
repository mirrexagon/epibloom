//! Straight generating from the LLM, mostly for testing.

use futures::{pin_mut, stream::StreamExt, Stream};
use rocket::{serde::json::Json, State};
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};

use crate::backend::{
    self,
    anthropic::{self, AnthropicApi},
    featherless::{self, FeatherlessApi},
    Backend,
};
use crate::chat::Chat;

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
#[openapi(paths(generate))]
pub struct BackendProxyApiDoc;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct GenerateResponse {
    thinking: Option<String>,
    message: String,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, ToSchema, FromFormField)]
#[serde(rename_all = "snake_case")]
pub enum GenerateBackend {
    Featherless,
    Claude,
}

/// Helper function to process a stream of chunks from any backend.
async fn process_stream<S, E>(stream: S) -> Result<Json<GenerateResponse>, ApiErrorResponse>
where
    S: Stream<Item = Result<backend::GenerateChunk, E>>,
    E: std::error::Error,
{
    pin_mut!(stream);

    let mut new_thinking = String::new();
    let mut new_message = String::new();

    while let Some(chunk) = stream.next().await {
        debug!("generate chunk: {chunk:#?}");

        match chunk {
            Ok(chunk) => match chunk {
                backend::GenerateChunk::ThinkingDelta(delta) => new_thinking.push_str(&delta),
                backend::GenerateChunk::MessageDelta(delta) => new_message.push_str(&delta),
            },
            Err(err) => return Err(ApiErrorResponse::BadGateway(err.to_string().to_owned())),
        }
    }

    Ok(Json(GenerateResponse {
        thinking: if new_thinking.is_empty() {
            None
        } else {
            Some(new_thinking)
        },
        message: new_message,
    }))
}

/// Generates an assistant response on the end of a chat using Featherless.
#[utoipa::path(
    responses(
        (status = 200, description = "Message generated succesfully.", body = GenerateResponse),
        (status = 502, description = "LLM backend error.", body = String)
    ),
    params(
        ("backend" = inline(GenerateBackend), Query, description = "The LLM backend/API to use to generate the message")
    )
)]
#[post("/generate?<backend>", data = "<chat>")]
pub async fn generate(
    chat: Json<Chat>,
    backend: GenerateBackend,
    featherless_api: &State<FeatherlessApi>,
    anthropic_api: &State<AnthropicApi>,
) -> Result<Json<GenerateResponse>, ApiErrorResponse> {
    match backend {
        GenerateBackend::Featherless => {
            let featherless_config = featherless::Config::default();
            let stream = featherless_api.generate(&featherless_config, &chat);
            process_stream(stream).await
        }

        GenerateBackend::Claude => {
            let anthropic_config = anthropic::Config {
                max_tokens: 2048,
                thinking_budget: Some(1024),
                ..Default::default()
            };

            // TODO for continue: Anthropic API returned error
            // 'invalid_request_error': messages: final assistant content cannot
            // end with trailing whitespace
            let stream = anthropic_api.generate(&anthropic_config, &chat);
            process_stream(stream).await
        }
    }
}
