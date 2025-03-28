#[macro_use]
extern crate rocket;

mod anthropic;
mod api;
mod chat;
mod util;

use std::{fs, path::PathBuf, sync::Mutex};

use etcetera::{choose_app_strategy, AppStrategy, AppStrategyArgs};

use chat::Chats;
use rusqlite::Connection;

const DATABASE_FILENAME: &str = "epibloom.db";

#[launch]
fn rocket() -> _ {
    env_logger::init();

    // TODO: Store API keys in database or config file.
    dotenvy::dotenv().unwrap();

    let strategy = choose_app_strategy(AppStrategyArgs {
        top_level_domain: "dev".to_string(),
        author: "Mirx".to_string(),
        app_name: "Epibloom".to_string(),
    })
    .unwrap();

    let config_dir = strategy.config_dir();
    let data_dir = strategy.data_dir();
    let cache_dir = strategy.cache_dir();
    let state_dir = strategy.state_dir();
    let runtime_dir = strategy.runtime_dir();

    fs::create_dir_all(&data_dir).unwrap();

    let mut db_path = PathBuf::new();
    db_path.push(&data_dir);
    db_path.push(DATABASE_FILENAME);

    let chats = {
        let db_conn = Connection::open(db_path.as_path()).unwrap();
        let chats = Chats::new(db_conn).unwrap();
        Mutex::new(chats)
    };

    let anthropic_api = anthropic::AnthropicApi::new_with_key_from_env();

    rocket::build()
        .mount(
            "/api/chat",
            routes![
                api::chat::get_chats,
                api::chat::get_chat_by_id,
                api::chat::new_chat,
                api::chat::push_message,
            ],
        )
        .mount("/api/anthropic", routes![api::list_anthropic_models])
        .manage(chats)
        .manage(anthropic_api)
}
