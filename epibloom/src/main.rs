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
