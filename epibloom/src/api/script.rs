use std::path::PathBuf;

use rocket::serde::json::Json;
use rocket::State;

use super::ApiErrorResponse;
use crate::script;

impl From<script::Error> for ApiErrorResponse {
    fn from(value: script::Error) -> Self {
        match value {
            script::Error::ScriptPathEscapesScriptDirectory => {
                ApiErrorResponse::Forbidden(value.to_string())
            }
            script::Error::Io(_) | script::Error::Lua(_) | script::Error::Json(_) => {
                ApiErrorResponse::InternalServerError(value.to_string())
            }
        }
    }
}

#[post("/<context_name>/<script_path>", data = "<script_input>")]
pub async fn run_script(
    context_name: &str,
    script_path: PathBuf,
    mut script_input: Json<serde_json::Value>,
    script_context_store: &State<script::ContextStore>,
) -> Result<serde_json::Value, ApiErrorResponse> {
    log::info!("Requested to run script {script_path:?} in context {context_name}");

    let context = script_context_store.get_context(context_name).await?;
    let result = context
        .run_script(&script_path, script_input.take())
        .await?;

    Ok(result)
}
