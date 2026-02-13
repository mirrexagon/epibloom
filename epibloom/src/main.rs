#![warn(clippy::clone_on_ref_ptr)]

#[macro_use]
extern crate rocket;

mod api;
mod backend;
mod script;
mod state;
mod trace;

use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use etcetera::{choose_app_strategy, AppStrategy, AppStrategyArgs};
use rocket::fairing::AdHoc;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

static TRACES_DIR: OnceLock<PathBuf> = OnceLock::new();
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

    TRACES_DIR.set(data_dir.join("traces")).unwrap();
    SCRIPTS_DIR.set(config_dir.join("scripts")).unwrap();
    SCRIPT_DATA_DIR.set(data_dir.join("data")).unwrap();

    fs::create_dir_all(TRACES_DIR.get().unwrap()).unwrap();
    fs::create_dir_all(SCRIPTS_DIR.get().unwrap()).unwrap();
    fs::create_dir_all(SCRIPT_DATA_DIR.get().unwrap()).unwrap();

    let trace_store = Arc::new(trace::TraceStore::new());
    let state_store = Arc::new(state::StateStore::new());
    let script_context_store =
        script::ContextStore::new(Arc::clone(&trace_store), Arc::clone(&state_store));

    #[derive(OpenApi)]
    #[openapi(
        nest(
            (path = "/api/trace", api = api::trace::TraceApiDoc),
            (path = "/api/state", api = api::state::StateApiDoc)
        ),
        tags(
            (name = "trace", description = "Streaming base debug traces, created and manipulated by scripts."),
            (name = "state", description = "Streaming state objects, created and manipulated by scripts.")
        )
    )]
    struct ApiDoc;

    rocket::build()
        .manage(trace_store)
        .manage(state_store)
        .manage(script_context_store)
        .attach(AdHoc::try_on_ignite("Load traces from files", |rocket| {
            Box::pin(async move {
                let trace_store: &Arc<trace::TraceStore> = match rocket.state() {
                    Some(trace_store) => trace_store,
                    None => return Err(rocket),
                };

                match trace_store.load_traces_from_traces_directory().await {
                    Ok(_) => {}
                    Err(err) => {
                        log::error!("Couldn't load traces: {}", err);
                        return Err(rocket);
                    }
                }

                log::info!("Loaded traces succesfully");

                Ok(rocket)
            })
        }))
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
        .mount(
            "/api/trace",
            routes![
                api::trace::stream_trace_store_events,
                api::trace::stream_trace_by_id,
            ],
        )
        .mount("/api/script", routes![api::script::run_script,])
        .mount("/api/state", routes![api::state::stream_state_objects,])
}
