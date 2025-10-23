//! Featherless AI backend.
//!
//! https://featherless.ai

mod chat_completion;

use std::env;

use futures::Stream;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::backends;

const FEATHERLESS_API_URL_BASE: &str = "https://api.featherless.ai/v1";
static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Config {
    max_tokens: u32,
    model: String,

    #[serde(default)]
    temperature: Option<f32>,
    #[serde(default)]
    top_p: Option<f32>,
    #[serde(default)]
    top_k: Option<i32>,
    #[serde(default)]
    stop: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_tokens: 2048,
            model: "zai-org/GLM-4.6".to_string(),
            temperature: Some(0.7),
            top_p: Some(0.9),
            top_k: None,
            stop: Vec::new(),
        }
    }
}

pub struct FeatherlessApi {
    client: reqwest::Client,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Featherless API returned error: {message}")]
    Api { message: String },
    #[error("HTTP client error: {0}")]
    HttpClient(#[from] reqwest::Error),
    #[error("Invalid response format: {0}")]
    InvalidResponse(String),
}

type Result<T> = std::result::Result<T, Error>;
impl FeatherlessApi {
    pub fn new_with_key_from_env() -> Result<Self> {
        use reqwest::{
            header::{self, HeaderMap, HeaderValue},
            Client,
        };

        let api_key = env::var("FEATHERLESS_API_KEY").unwrap();

        let mut default_headers = HeaderMap::new();

        default_headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {api_key}")).unwrap(),
        );
        default_headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );

        let client = Client::builder()
            .user_agent(APP_USER_AGENT)
            .default_headers(default_headers)
            .build()?;

        Ok(FeatherlessApi { client })
    }
}

impl super::Backend for FeatherlessApi {
    type Config = Config;
    type Error = Error;

    fn generate<'a>(
        &'a self,
        config: &'a Self::Config,
        chat: &'a backends::Chat,
    ) -> impl Stream<Item = std::result::Result<backends::GenerateChunk, Self::Error>> + 'a {
        chat_completion::generate(&self.client, config, chat)
    }
}
