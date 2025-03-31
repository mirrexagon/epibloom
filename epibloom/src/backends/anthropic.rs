mod messages;
mod models;

use std::env;

use futures::stream::StreamExt;
use log::{info, trace};
use serde::{Deserialize, Serialize};
use sse_stream::SseStream;
use time::OffsetDateTime;

use crate::data;

// https://docs.anthropic.com/en/api/versioning
const ANTHROPIC_API_VERSION: &str = "2023-06-01";
const ANTHROPIC_API_URL_BASE: &str = "https://api.anthropic.com/v1";

static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

pub struct AnthropicApi {
    api_key: String,
    client: reqwest::Client,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),
}

type Result<T> = std::result::Result<T, Error>;

impl AnthropicApi {
    pub fn new_with_key_from_env() -> Result<Self> {
        use reqwest::{
            header::{self, HeaderMap, HeaderName, HeaderValue},
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

        Ok(AnthropicApi { api_key, client })
    }

    pub async fn list_models(&self) -> Result<Vec<models::ModelInfo>> {
        models::list_models(&self.client).await.map_err(From::from)
    }

    pub async fn generate(&self, chat: &data::chat::Chat) -> Result<String> {
        messages::generate_stream(&self.client, chat)
            .await
            .map_err(From::from)
    }
}

pub mod api {
    /*
    use rocket::{get, http::Status, response::status, serde::json::Json, State};

    use super::*;

    #[get("/models")]
    pub async fn list_models(
        anthropic_api: &State<AnthropicApi>,
    ) -> Result<Json<Vec<ModelInfo>>, status::Custom<String>> {
        let model_list = anthropic_api
            .list_models()
            .await
            .map_err(|err| {
                status::Custom(
                    Status::BadGateway,
                    format!("Failed to get model list from Anthropic: {err}"),
                )
            })
            .map_err(|err| err.to_string());

        Ok(Json(model_list))
    }
    */
}
