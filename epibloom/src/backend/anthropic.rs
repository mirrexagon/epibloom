use std::env;
use std::pin::Pin;

use futures::Stream;
use schemars::JsonSchema;
use serde::Deserialize;

pub mod messages;
mod models;

// https://docs.anthropic.com/en/api/versioning
const ANTHROPIC_API_VERSION: &str = "2023-06-01";
const ANTHROPIC_API_URL_BASE: &str = "https://api.anthropic.com/v1";

static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

#[derive(Clone)]
pub struct AnthropicClient {
    client: reqwest::Client,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Anthropic API returned error '{type}': {message}")]
    Api { r#type: String, message: String },

    #[error("streaming event has no data")]
    StreamingEventHasNoData,

    #[error("Server-sent event error: {0}")]
    Sse(#[from] sse_stream::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("HTTP client error: {0}")]
    HttpClient(#[from] reqwest::Error),

    #[error("Environment variable error: {0}")]
    EnvVar(#[from] std::env::VarError),
}

impl AnthropicClient {
    pub fn new_from_env() -> Result<Self, Error> {
        use reqwest::{
            header::{self, HeaderMap, HeaderValue},
            Client,
        };

        let api_key = env::var("ANTHROPIC_API_KEY")?;

        let mut default_headers = HeaderMap::new();

        default_headers.insert("x-api-key", HeaderValue::from_str(&api_key).unwrap());
        default_headers.insert(
            "anthropic-version",
            HeaderValue::from_static(ANTHROPIC_API_VERSION),
        );
        default_headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );

        let client = Client::builder()
            .user_agent(APP_USER_AGENT)
            .default_headers(default_headers)
            .build()?;

        Ok(AnthropicClient { client })
    }

    pub async fn list_models(&self) -> Result<Vec<models::ModelInfo>, Error> {
        models::list_models(&self.client).await
    }

    pub fn generate(
        &self,
        request: messages::request::Request,
    ) -> Pin<Box<dyn Stream<Item = Result<messages::response::StreamingEvent, Error>> + Send>> {
        Box::pin(messages::generate(self.client.clone(), request))
    }
}
