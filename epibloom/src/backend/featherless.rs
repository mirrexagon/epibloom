//! Featherless AI backend.
//!
//! https://featherless.ai

mod chat_completion;

use std::env;
use std::pin::Pin;

use futures::Stream;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::backend;

const FEATHERLESS_API_URL_BASE: &str = "https://api.featherless.ai/v1";
static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

#[derive(Debug, Deserialize, JsonSchema, Clone)]
pub struct Config {
    pub max_tokens: u32,
    pub model: String,

    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub top_p: Option<f32>,
    #[serde(default)]
    pub top_k: Option<i32>,
    #[serde(default)]
    pub stop_sequences: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_tokens: 512,
            model: "zai-org/GLM-4.6".to_string(),
            temperature: Some(0.7),
            top_p: Some(0.9),
            top_k: None,
            stop_sequences: Vec::new(),
        }
    }
}

#[derive(Clone)]
pub struct FeatherlessApi {
    client: reqwest::Client,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Featherless API returned error: {message}")]
    Api { message: String },

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

type Result<T> = std::result::Result<T, Error>;
impl FeatherlessApi {}

impl super::Backend for FeatherlessApi {
    type Config = Config;
    type Error = Error;

    fn new_with_key_from_env() -> Result<Self> {
        use reqwest::{
            header::{self, HeaderMap, HeaderValue},
            Client,
        };

        let api_key = env::var("FEATHERLESS_API_KEY")?;

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

    fn generate(
        &self,
        config: &Self::Config,
        trace: &backend::Trace,
    ) -> Pin<Box<dyn Stream<Item = std::result::Result<backend::GenerateChunk, Self::Error>> + Send>>
    {
        // TODO: See if we can get away with not cloning here, since technically
        // the stream should have no lifetime attachment to anything passed in,
        // they are just used to start the generation I thought.
        // But maybe Chat is kept since it is the basis for the request. Still,
        // the request should be sent and no longer used for the stream.
        Box::pin(chat_completion::generate(
            self.clone().client,
            config.clone(),
            trace.clone(),
        ))
    }
}
