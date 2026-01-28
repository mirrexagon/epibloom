use rocket::State;

use super::ApiErrorResponse;

#[post("/<context_name>/<script_name>")]
pub async fn test_generate(
    context_name: &str,
    script_name: &str,
) -> Result<serde_json::Value, ApiErrorResponse> {
    log::info!("Requested to run script {script_name} in context {context_name}");

    todo!();
}
