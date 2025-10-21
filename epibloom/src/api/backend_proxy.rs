//! Straight generating from the LLM, mostly for testing.

use futures::{pin_mut, stream::StreamExt};
use rocket::{serde::json::Json, State};
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};

use crate::backends::{
    self,
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
#[openapi(paths(generate))]
pub struct BackendProxyApiDoc;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct GenerateResponse {
    message: String,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, ToSchema, FromFormField)]
pub enum GenerateBackend {
    Featherless,
    Claude,
}

/// Generates an assistant response on the end of a chat using Featherless.
#[utoipa::path(
    responses(
        (status = 200, description = "Message generated succesfully.", body = GenerateResponse),
        (status = 502, description = "LLM backend error.", body = String)
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

            // TODO: Figure out how to not have to copy the below stuff into
            // each backend branch because they each return a different concrete
            // Stream type.
            pin_mut!(stream);

            let mut new_message = String::new();

            while let Some(chunk) = stream.next().await {
                debug!("generate chunk: {chunk:#?}");

                match chunk {
                    Ok(chunk) => match chunk {
                        backends::GenerateChunk::MessageDelta(delta) => {
                            new_message.push_str(&delta)
                        }
                    },
                    Err(err) => {
                        return Err(ApiErrorResponse::BadGateway(err.to_string().to_owned()))
                    }
                }
            }

            Ok(Json(GenerateResponse {
                message: new_message,
            }))
        }

        GenerateBackend::Claude => {
            let anthropic_config = anthropic::Config::default();

            // TODO for continue: Anthropic API returned error
            // 'invalid_request_error': messages: final assistant content cannot
            // end with trailing whitespace
            let stream = anthropic_api.generate(&anthropic_config, &chat);
            pin_mut!(stream);

            let mut new_message = String::new();

            while let Some(chunk) = stream.next().await {
                debug!("generate chunk: {chunk:#?}");

                match chunk {
                    Ok(chunk) => match chunk {
                        backends::GenerateChunk::MessageDelta(delta) => {
                            new_message.push_str(&delta)
                        }
                    },
                    Err(err) => {
                        return Err(ApiErrorResponse::BadGateway(err.to_string().to_owned()))
                    }
                }
            }

            Ok(Json(GenerateResponse {
                message: new_message,
            }))
        }
    }
}
