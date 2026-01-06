#[macro_use]
extern crate rocket;

mod api;
mod backends;
mod chat;
mod util;

use std::fs;
use std::path::PathBuf;

use etcetera::{choose_app_strategy, AppStrategy, AppStrategyArgs};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use backends::anthropic::AnthropicApi;
use backends::featherless::FeatherlessApi;

const DATABASE_FILENAME: &str = "epibloom.db";

#[launch]
fn rocket() -> _ {
    env_logger::init();

    // TODO: Store API keys in database or config file.
    dotenvy::dotenv().expect("Could not open .env file");

    let strategy = choose_app_strategy(AppStrategyArgs {
        top_level_domain: "dev".to_string(),
        author: "Mirx".to_string(),
        app_name: "Epibloom".to_string(),
    })
    .unwrap();

    let _config_dir = strategy.config_dir();
    let data_dir = strategy.data_dir();
    let _cache_dir = strategy.cache_dir();
    let _state_dir = strategy.state_dir();
    let _runtime_dir = strategy.runtime_dir();

    fs::create_dir_all(&data_dir).unwrap();

    let chat_session_store = chat::ChatStore::new();

    let anthropic_api: AnthropicApi = AnthropicApi::new_with_key_from_env().unwrap();
    let featherless_api: FeatherlessApi = FeatherlessApi::new_with_key_from_env().unwrap();

    #[derive(OpenApi)]
    #[openapi(
        nest(
            (path = "/api/backend-proxy", api = api::backend_proxy::BackendProxyApiDoc),
            (path = "/api/chat", api = api::chat::ChatApiDoc)
        ),
        tags(
            (name = "backend-proxy", description = "Simple proxy to the LLM backends."),
            (name = "chat", description = "Streaming internal chats used to drive the game.")
        )
    )]
    struct ApiDoc;

    rocket::build()
        .manage(anthropic_api)
        .manage(featherless_api)
        .manage(chat_session_store)
        .mount(
            "/",
            SwaggerUi::new("/swagger-ui/<_..>").url("/api-docs/openapi.json", ApiDoc::openapi()),
        )
        .mount("/api/backend-proxy", routes![api::backend_proxy::generate])
        .mount(
            "/api/chat",
            routes![
                api::chat::get_chats,
                api::chat::stream_chat_by_id,
                api::chat::test_generate
            ],
        )
}
