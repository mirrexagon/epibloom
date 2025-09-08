#[macro_use]
extern crate rocket;

mod api;
mod backends;
mod data;
mod util;

use std::{fs, path::PathBuf, sync::Mutex};

use etcetera::{choose_app_strategy, AppStrategy, AppStrategyArgs};
use rusqlite::Connection;
use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;

use backends::anthropic::AnthropicApi;
use data::chat::Chats;

const DATABASE_FILENAME: &str = "epibloom.db";

// TODO: Each backend has functions that take a RenderedChat and send it to the backend
// and return the response message.
// System prompt in RenderedChat should be separate, like Anthropic API, not a message
// in the chat.
// How would this interact with lorebooks and other injections? Probably
// post-processed chat that includes relevant lorebook info.
// How would this interact with tool use? Needs to call back into Epibloom to
// run the tool. See
// https://docs.anthropic.com/en/docs/build-with-claude/tool-use/overview,
// Claude will stop generating and say to use the tool, then you run a
// continuation.
//
// TODO: For characters/system prompts, world info, etc., chats can be linked
// to a single character and to 0 or more lorebooks. Code somewhere (maybe in
// data::chat or some chat builder module) will read these and build a final
// temporary backend::Chat that then each backend knows how to send to its
// API/add instruct tokens and turn into a text blob for text completion/etc.

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

    let mut db_path = PathBuf::new();
    db_path.push(&data_dir);
    db_path.push(DATABASE_FILENAME);

    let chats = {
        let db_conn = Connection::open(db_path.as_path()).unwrap();
        let chats = Chats::new(db_conn).unwrap();
        Mutex::new(chats)
    };

    let anthropic_api: AnthropicApi = AnthropicApi::new_with_key_from_env().unwrap();

    #[derive(OpenApi)]
    #[openapi(
        nest(
            (path = "/api/chat", api = api::chat::ChatApiDoc)
        ),
        tags(
            (name = "chat", description = "Chat management and interaction.")
        )
    )]
    struct ApiDoc;

    rocket::build()
        .manage(chats)
        .manage(anthropic_api)
        .mount(
            "/",
            SwaggerUi::new("/swagger-ui/<_..>").url("/api-docs/openapi.json", ApiDoc::openapi()),
        )
        .mount(
            "/api/chat",
            routes![
                api::chat::get_chats,
                api::chat::get_chat_by_id,
                api::chat::new_chat,
                api::chat::push_message,
                api::chat::generate,
            ],
        )
}
