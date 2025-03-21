#[macro_use]
extern crate rocket;

mod anthropic;
mod api;
mod chat;

use std::sync::Mutex;


use chat::Chats;

#[launch]
fn rocket() -> _ {
    env_logger::init();
    dotenvy::dotenv().unwrap();

    let chats = Chats::new();
    let chats = Mutex::new(chats);

    let anthropic_api = anthropic::AnthropicApi::new_with_key_from_env();

    rocket::build()
        .mount(
            "/api",
            routes![
                api::get_chats,
                api::get_chat_by_id,
                api::new_chat,
                api::list_anthropic_models
            ],
        )
        .manage(chats)
        .manage(anthropic_api)
}
