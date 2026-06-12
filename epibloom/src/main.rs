#![warn(clippy::clone_on_ref_ptr)]

#[macro_use]
extern crate rocket;

mod api;
mod backend;
mod llm;
mod script;
mod state;
mod util;

use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use etcetera::{choose_app_strategy, AppStrategy, AppStrategyArgs};
use rocket::fairing::AdHoc;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

static SCRIPTS_DIR: OnceLock<PathBuf> = OnceLock::new();
static STATE_PERSIST_DIR: OnceLock<PathBuf> = OnceLock::new();

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

    SCRIPTS_DIR.set(config_dir.join("scripts")).unwrap();
    STATE_PERSIST_DIR.set(data_dir.join("state")).unwrap();

    fs::create_dir_all(SCRIPTS_DIR.get().unwrap()).unwrap();
    fs::create_dir_all(STATE_PERSIST_DIR.get().unwrap()).unwrap();

    let state_store = Arc::new(state::StateStore::new());
    let llm_store = Arc::new(llm::LlmStore::new(Arc::clone(&state_store)));
    let script_context_store =
        script::ContextStore::new(Arc::clone(&llm_store), Arc::clone(&state_store));

    #[derive(OpenApi)]
    #[openapi(
        nest(
            (path = "/api/state", api = api::state::StateApiDoc),
            (path = "/api/script", api = api::script::ScriptApiDoc),
        ),
        tags(
            (name = "api::state", description = "Streaming and persistable state objects, created and manipulated by scripts."),
            (name = "api::script", description = "Running Lua scripts to perform actions and update state."),
        )
    )]
    struct ApiDoc;

    rocket::build()
        .manage(state_store)
        .manage(llm_store)
        .manage(script_context_store)
        .attach(AdHoc::on_liftoff("Log location of API docs", |rocket| {
            Box::pin(async move {
                let config = rocket.config();
                let address = config.address;
                let port = config.port;

                log::info!(
                    "See API docs at http://{address}:{port}/swagger-ui/\nOpenAPI description: http://{address}:{port}/api-docs/openapi.json",
                );
            })
        }))
        .mount(
            "/",
            SwaggerUi::new("/swagger-ui/<_..>").url("/api-docs/openapi.json", ApiDoc::openapi()),
        )
        .mount("/api/state", routes![api::state::stream_state_objects,])
        .mount("/api/script", routes![api::script::run_script,])
}
