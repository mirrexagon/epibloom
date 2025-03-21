use rocket::{get, http::Status, response::status, serde::json::Json, State};

use crate::anthropic;

pub mod chat;

#[get("/models")]
pub async fn list_anthropic_models(
    anthropic_api: &State<anthropic::AnthropicApi>,
) -> Result<Json<Vec<anthropic::ModelInfo>>, status::Custom<String>> {
    let model_list = anthropic_api.list_models().await.map_err(|err| {
        status::Custom(
            Status::BadGateway,
            format!("Failed to get model list from Anthropic: {err}"),
        )
    })?;

    Ok(Json(model_list))
}
