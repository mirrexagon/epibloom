//! Types and functions for the /models endpoint.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::ANTHROPIC_API_URL_BASE;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModelInfoPage {
    data: Vec<ModelInfo>,
    first_id: String,
    last_id: String,
    has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    id: String,
    display_name: String,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
    capabilities: ModelCapabilities,
    max_input_tokens: u32,
    max_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCapabilities {
    batch: CapabilitySupport,
    citations: CapabilitySupport,
    code_execution: CapabilitySupport,
    context_management: ContextManagementCapability,
    effort: EffortCapability,
    image_input: CapabilitySupport,
    pdf_input: CapabilitySupport,
    structured_outputs: CapabilitySupport,
    thinking: ThinkingCapability,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilitySupport {
    supported: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextManagementCapability {
    supported: bool,
    clear_thinking_20251015: CapabilitySupport,
    clear_tool_uses_20250919: CapabilitySupport,
    compact_20260112: CapabilitySupport,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffortCapability {
    supported: bool,
    low: CapabilitySupport,
    medium: CapabilitySupport,
    high: CapabilitySupport,
    xhigh: CapabilitySupport,
    max: CapabilitySupport,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingCapability {
    types: ThinkingTypes,
    supported: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingTypes {
    enabled: CapabilitySupport,
    adaptive: CapabilitySupport,
}

pub async fn list_models(client: &reqwest::Client) -> Result<Vec<ModelInfo>, super::Error> {
    let mut model_list = Vec::new();

    // Get first page.
    let url = format!("{ANTHROPIC_API_URL_BASE}/models");
    let response = client.get(url).send().await?;
    let mut model_info_page: ModelInfoPage = response.json().await?;
    model_list.append(&mut model_info_page.data);

    while model_info_page.has_more {
        let url = format!(
            "{ANTHROPIC_API_URL_BASE}/models?after_id={}",
            model_info_page.last_id
        );

        let response = client.get(url).send().await?;
        model_info_page = response.json().await?;
        model_list.append(&mut model_info_page.data);
    }

    Ok(model_list)
}
