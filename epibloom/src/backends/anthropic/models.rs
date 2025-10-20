//! Types and functions for the /models endpoint.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::ANTHROPIC_API_URL_BASE;

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

pub async fn list_models(client: &reqwest::Client) -> super::Result<Vec<ModelInfo>> {
    let url = format!("{ANTHROPIC_API_URL_BASE}/models");

    let response = client.get(url).send().await?;
    let body: ModelInfoPage = response.json().await?;

    // TODO: Concatenate all pages in the model list.
    Ok(body.data)
}
