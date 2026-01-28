#[macro_use]
extern crate rocket;

mod api;
mod backend;
mod chat;
mod script;

use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

use etcetera::{choose_app_strategy, AppStrategy, AppStrategyArgs};
use rocket::fairing::AdHoc;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use backend::anthropic::AnthropicApi;
use backend::featherless::FeatherlessApi;

static CHATS_DIR: OnceLock<PathBuf> = OnceLock::new();

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

    CHATS_DIR.set(data_dir.join("chats")).unwrap();

    fs::create_dir_all(&data_dir).unwrap();
    fs::create_dir_all(CHATS_DIR.get().unwrap()).unwrap();

    let chat_store = chat::ChatStore::new();

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
        .manage(chat_store)
        .attach(AdHoc::try_on_ignite("Load chats from files", |rocket| {
            Box::pin(async move {
                let chat_store: &chat::ChatStore = match rocket.state() {
                    Some(chat_store) => chat_store,
                    None => return Err(rocket),
                };

                match chat_store.load_chats_from_chats_directory().await {
                    Ok(_) => {}
                    Err(err) => {
                        log::error!("Couldn't load chats: {}", err);
                        return Err(rocket);
                    }
                }

                log::info!("Loaded chats succesfully");

                Ok(rocket)
            })
        }))
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
