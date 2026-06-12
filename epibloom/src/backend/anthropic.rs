use std::env;
use std::pin::Pin;

use futures::Stream;

use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

pub mod messages;
pub mod models;

// https://docs.anthropic.com/en/api/versioning
const ANTHROPIC_API_VERSION: &str = "2023-06-01";
const ANTHROPIC_API_URL_BASE: &str = "https://api.anthropic.com/v1";

#[derive(Clone)]
pub struct AnthropicClient {
    client: reqwest::Client,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Anthropic API returned error: {0:?}")]
    Api(ApiErrorObject),

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorObject {
    error: ApiError,
    request_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    r#type: ApiErrorType,
    message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiErrorType {
    /// HTTP response code 400 - There was an issue with the format or content
    /// of your request. This error type may also be used for other 4XX status
    /// codes not listed here.
    #[serde(rename = "invalid_request_error")]
    InvalidRequest,

    /// HTTP response code 401 - There's an issue with your API key.
    #[serde(rename = "authentication_error")]
    Authentication,

    /// HTTP response code 402 - There's an issue with your billing or payment
    /// information. Check your payment details in the Claude Console.
    #[serde(rename = "billing_error")]
    Billing,

    /// HTTP response code 403 - Your API key does not have permission to use
    /// the specified resource.
    #[serde(rename = "permission_error")]
    Permission,

    /// HTTP response code 404 - The requested resource was not found.
    #[serde(rename = "not_found_error")]
    NotFound,

    /// 413 - Request exceeds the maximum allowed number of bytes.
    #[serde(rename = "request_too_large")]
    RequestTooLarge,

    /// HTTP response code 429 - Your account has hit a rate limit.
    #[serde(rename = "rate_limit_error")]
    RateLimit,

    /// HTTP response code 500 - An unexpected error has occurred internal to
    /// Anthropic's systems.
    #[serde(rename = "api_error")]
    Api,

    /// HTTP response code 504 - The request timed out while processing.
    /// Consider using streaming for long-running requests.
    #[serde(rename = "timeout_error")]
    Timeout,

    /// HTTP response code 529 - The API is temporarily overloaded.
    #[serde(rename = "overloaded_error")]
    Overloaded,
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
            .user_agent(crate::APP_USER_AGENT)
            .default_headers(default_headers)
            .build()?;

        Ok(AnthropicClient { client })
    }

    pub async fn list_models(&self) -> Result<Vec<models::ModelInfo>, Error> {
        models::list_models(&self.client).await
    }

    pub async fn generate(
        &self,
        request: messages::request::MessagesRequest,
    ) -> Result<messages::response::Message, Error> {
        messages::generate(self.client.clone(), request).await
    }

    pub fn generate_streaming(
        &self,
        request: messages::request::MessagesRequest,
    ) -> Pin<Box<dyn Stream<Item = Result<messages::response::MessageStreamEvent, Error>> + Send>>
    {
        Box::pin(messages::generate_streaming(self.client.clone(), request))
    }

    pub async fn count_tokens(
        &self,
        request: messages::request::CountTokensRequest,
    ) -> Result<messages::response::MessageTokensCount, Error> {
        messages::count_tokens(self.client.clone(), request).await
    }
}

trait AnthropicReqwestClient {
    async fn anthropic_get(&self, path: &str) -> Result<reqwest::Response, Error>;
    async fn anthropic_post<T>(&self, path: &str, body: &T) -> Result<reqwest::Response, Error>
    where
        T: Serialize + ?Sized;
}

impl AnthropicReqwestClient for reqwest::Client {
    async fn anthropic_get(&self, path: &str) -> Result<reqwest::Response, Error> {
        let url = format!("{ANTHROPIC_API_URL_BASE}/{path}");
        let response = self.get(url).send().await?;
        if response.status() == StatusCode::OK {
            Ok(response)
        } else {
            Err(Error::Api(response.json().await?))
        }
    }

    async fn anthropic_post<T>(&self, path: &str, body: &T) -> Result<reqwest::Response, Error>
    where
        T: Serialize + ?Sized,
    {
        log::debug!(
            "POST to {path} body: {}",
            serde_json::to_string_pretty(body)?
        );

        let url = format!("{ANTHROPIC_API_URL_BASE}/{path}");
        let response = self.post(url).json(body).send().await?;
        if response.status() == StatusCode::OK {
            Ok(response)
        } else {
            Err(Error::Api(response.json().await?))
        }
    }
}
