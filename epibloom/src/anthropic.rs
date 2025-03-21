use std::env;

use log::info;
use rocket::serde::{Deserialize, Serialize};
use time::OffsetDateTime;

// https://docs.anthropic.com/en/api/versioning
const ANTHROPIC_API_VERSION: &str = "2023-06-01";
const ANTHROPIC_API_URL_BASE: &str = "https://api.anthropic.com/v1";

pub struct AnthropicApi {
    api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct ModelInfo {
    r#type: String,
    id: String,
    display_name: String,
    #[serde(with = "time::serde::iso8601")]
    created_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
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
}
