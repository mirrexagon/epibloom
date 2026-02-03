#[macro_use]
extern crate rocket;

mod api;
mod backend;
mod chat;
mod script;

use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use etcetera::{choose_app_strategy, AppStrategy, AppStrategyArgs};
use rocket::fairing::AdHoc;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

static DEBUG_CHATS_DIR: OnceLock<PathBuf> = OnceLock::new();
static SCRIPTS_DIR: OnceLock<PathBuf> = OnceLock::new();
static SCRIPT_DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

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

    let config_dir = strategy.config_dir();
    let data_dir = strategy.data_dir();
    let _cache_dir = strategy.cache_dir();
    let _state_dir = strategy.state_dir();
    let _runtime_dir = strategy.runtime_dir();

    DEBUG_CHATS_DIR.set(data_dir.join("debugchats")).unwrap();
    SCRIPTS_DIR.set(config_dir.join("scripts")).unwrap();
    SCRIPT_DATA_DIR.set(data_dir.join("data")).unwrap();

    fs::create_dir_all(DEBUG_CHATS_DIR.get().unwrap()).unwrap();
    fs::create_dir_all(SCRIPTS_DIR.get().unwrap()).unwrap();
    fs::create_dir_all(SCRIPT_DATA_DIR.get().unwrap()).unwrap();

    let chat_store = Arc::new(chat::ChatStore::new());
    let script_context_store = script::ContextStore::new(Arc::clone(&chat_store));

    #[derive(OpenApi)]
    #[openapi(
        nest(
            (path = "/api/chat", api = api::chat::ChatApiDoc)
        ),
        tags(
            (name = "chat", description = "Streaming base debug chats, created and manipulated by scripts.")
        )
    )]
    struct ApiDoc;

    rocket::build()
        .manage(chat_store)
        .manage(script_context_store)
        .attach(AdHoc::try_on_ignite("Load chats from files", |rocket| {
            Box::pin(async move {
                let chat_store: &Arc<chat::ChatStore> = match rocket.state() {
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
        .mount(
            "/api/chat",
            routes![
                api::chat::stream_chat_store_events,
                api::chat::stream_chat_by_id,
            ],
        )
        .mount("/api/script", routes![api::script::run_script,])
}
