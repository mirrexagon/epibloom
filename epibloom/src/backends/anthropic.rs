mod messages;
mod models;

use std::env;

use schemars::{schema::RootSchema, schema_for, JsonSchema};
use serde::Deserialize;

use crate::data;

// https://docs.anthropic.com/en/api/versioning
const ANTHROPIC_API_VERSION: &str = "2023-06-01";
const ANTHROPIC_API_URL_BASE: &str = "https://api.anthropic.com/v1";

static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Config {
    max_tokens: u32,
    model: String,

    stop_sequences: Vec<String>,
    thinking: bool,
    thinking_budget: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_tokens: 512,
            model: "claude-3-haiku-20240307".to_string(),

            stop_sequences: Vec::new(),
            thinking: false,
            thinking_budget: 0,
        }
    }
}

pub struct AnthropicApi {
    client: reqwest::Client,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Anthropic API returned error '{type}': {message}")]
    Api { r#type: String, message: String },
    #[error("HTTP client error: {0}")]
    HttpClient(#[from] reqwest::Error),
}

type Result<T> = std::result::Result<T, Error>;

impl AnthropicApi {
    pub fn new_with_key_from_env() -> Result<Self> {
        use reqwest::{
            header::{self, HeaderMap, HeaderValue},
            Client,
        };

        let api_key = env::var("ANTHROPIC_API_KEY").unwrap();

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

        Ok(AnthropicApi { client })
    }

    pub async fn list_models(&self) -> Result<Vec<models::ModelInfo>> {
        models::list_models(&self.client).await
    }
}

impl super::Backend for AnthropicApi {
    type Config = Config;
    type Error = Error;

    async fn generate(
        &self,
        config: &Self::Config,
        chat: &data::chat::Chat,
    ) -> std::result::Result<String, Self::Error> {
        messages::generate(&self.client, config, chat).await
    }
}
