use std::env;

use log::info;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::chat;

// https://docs.anthropic.com/en/api/versioning
const ANTHROPIC_API_VERSION: &str = "2023-06-01";
const ANTHROPIC_API_URL_BASE: &str = "https://api.anthropic.com/v1";

pub struct AnthropicApi {
    api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    r#type: String,
    id: String,
    display_name: String,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModelInfoPage {
    data: Vec<ModelInfo>,
    has_more: bool,
    first_id: String,
    last_id: String,
}

impl AnthropicApi {
    pub fn new_with_key_from_env() -> Self {
        let api_key = env::var("ANTHROPIC_API_KEY").unwrap();

        AnthropicApi { api_key }
    }

    pub async fn list_models(&self) -> Result<Vec<ModelInfo>, String> {
        let client = reqwest::Client::new();
        let url = format!("{base}/models", base = ANTHROPIC_API_URL_BASE);

        let response = client
            .get(url)
            .header("x-api-key", self.api_key.clone())
            .header("anthropic-version", ANTHROPIC_API_VERSION)
            .header("content-type", "application/json")
            .send()
            .await
            .map_err(|err| err.to_string())?;

        //let text = response.text().await.map_err(|err| err.to_string())?;
        //debug!("{}", text);

        let body: ModelInfoPage = response.json().await.map_err(|err| err.to_string())?;
        info!("{:?}", body);

        // TODO: Concatenate all pages in the model list.
        Ok(body.data)
    }

    //pub async fn generate(&self, chat: chat::Chat) -> chat::Message {}
}

pub mod api {
    use rocket::{get, http::Status, response::status, serde::json::Json, State};

    use super::*;

    #[get("/models")]
    pub async fn list_models(
        anthropic_api: &State<AnthropicApi>,
    ) -> Result<Json<Vec<ModelInfo>>, status::Custom<String>> {
        let model_list = anthropic_api.list_models().await.map_err(|err| {
            status::Custom(
                Status::BadGateway,
                format!("Failed to get model list from Anthropic: {err}"),
            )
        })?;

        Ok(Json(model_list))
    }
}
