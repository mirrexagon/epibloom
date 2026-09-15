use std::path::PathBuf;

use rocket::serde::json::Json;
use rocket::State;
use utoipa::OpenApi;

use super::ApiErrorResponse;
use crate::script;

#[derive(OpenApi)]
#[openapi(paths(run_script))]
pub struct ScriptApiDoc;

// TODO: Report errors in a more structured way.
// Probably goes for all APIs.
// TODO: Report serialization/deserialization/etc. errors with position and key information if possible
impl From<script::Error> for ApiErrorResponse {
    fn from(value: script::Error) -> Self {
        match value {
            script::Error::ScriptPathEscapesScriptDirectory => {
                ApiErrorResponse::Forbidden(value.to_string())
            }
            script::Error::ScriptQueueFull
            | script::Error::Io(_)
            | script::Error::Lua(_)
            | script::Error::Json(_) => ApiErrorResponse::InternalServerError(value.to_string()),
        }
    }
}

/// Run a script and get the result back.
///
/// Scripts are run on script contexts, which are independent Lua states. Only
/// one script can run at a time on a single context, but multiple contexts can
/// have scripts running simultaneously. Multiple concurrent requests to run
/// scripts on a single context will run each one-by-one in the order of the
/// requests.
#[utoipa::path(
    responses(
        (status = 200, description = "The result of the script execution.", content_type = "text/json"),
    ),
    params(
        ("context_name" = String, description = "Name of the script context to run the script on."),
        ("script_path" = String, description = "Path to the script relative to the scripts directory, without a file extension."),
        (
            "script_input",
            description = "A JSON value that will be passed to the script.",
            example = json!({"hello": "world", "value": 6}),
        ),
    ),
)]
#[post("/<context_name>/<script_path..>", data = "<script_input>")]
pub async fn run_script(
    context_name: &str,
    script_path: PathBuf,
    mut script_input: Json<serde_json::Value>,
    script_context_store: &State<script::ContextStore>,
) -> Result<(), ApiErrorResponse> {
    let context = script_context_store.get_context(context_name).await?;
    let result = context
        .run_script(&script_path, script_input.take())
        .await?;

    Ok(())
}
