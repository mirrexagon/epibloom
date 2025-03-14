#[macro_use]
extern crate rocket;

mod anthropic;
mod chat;

mod api {
    use std::sync::Mutex;

    use rocket::serde::{Deserialize, Serialize};
    use rocket::{get, http::Status, response::status, serde::json::Json, State};
    use serde_json::json;
    use time::macros::format_description;
    use time::OffsetDateTime;
    use uuid::Uuid;

    use crate::anthropic;
    use crate::chat::{Chat, Chats};

    // TODO: Move Rocket-agnostic logic to impl functions on the chat structs.

    #[derive(Debug, Clone, Copy, Serialize, Deserialize)]
    #[serde(crate = "rocket::serde")]
    pub struct NewChatResponse {
        id: Uuid,
    }

    #[post("/chat")]
    pub fn new_chat(
        chats: &State<Mutex<Chats>>,
    ) -> Result<status::Created<Json<NewChatResponse>>, status::Custom<&'static str>> {
        let mut chats = chats.lock().unwrap();

        let new_chat_id = Uuid::new_v4();
        if chats.0.contains_key(&new_chat_id) {
            return Err(status::Custom(
                Status::InternalServerError,
                "Generated a duplicate chat ID",
            ));
        }

        let current_datetime = OffsetDateTime::now_local().unwrap();
        let dtfmt = format_description!(version = 2, "[year]-[month]-[day]T[hour][minute][second]");
        let new_chat_name = current_datetime.format(&dtfmt).unwrap();
        chats.0.insert(new_chat_id, Chat::new(&new_chat_name));

        let response = NewChatResponse { id: new_chat_id };
        let response =
            status::Created::new(format!("/chat/{id}", id = new_chat_id)).body(Json(response));

        Ok(response)
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(crate = "rocket::serde")]
    pub struct GetChatsResponseChat {
        id: Uuid,
        name: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(crate = "rocket::serde")]
    pub struct GetChatsResponse {
        chats: Vec<GetChatsResponseChat>,
    }

    #[get("/chat")]
    pub fn get_chats(chats: &State<Mutex<Chats>>) -> Json<GetChatsResponse> {
        let chats = chats.lock().unwrap();

        let mut response = GetChatsResponse { chats: Vec::new() };

        for (chat_id, chat) in chats.0.iter() {
            response.chats.push(GetChatsResponseChat {
                id: chat_id.clone(),
                name: chat.name.to_owned(),
            });
        }

        Json(response)
    }

    #[get("/chat/<id>")]
    pub fn get_chat_by_id(
        chats: &State<Mutex<Chats>>,
        id: Uuid,
    ) -> Result<Json<Chat>, status::NotFound<()>> {
        let chats = chats.lock().unwrap();

        if let Some(chat) = chats.0.get(&id) {
            let chat = chat.clone();
            return Ok(Json(chat));
        }

        Err(status::NotFound(()))
    }

    #[get("/anthropic/models")]
    pub async fn list_anthropic_models(
        anthropic_api: &State<anthropic::AnthropicApi>,
    ) -> Result<Json<Vec<anthropic::ModelInfo>>, status::Custom<String>> {
        let model_list = anthropic_api.list_models().await.map_err(|err| {
            status::Custom(
                Status::BadGateway,
                format!("Failed to get model list from Anthropic: {err}"),
            )
        })?;

        Ok(Json(model_list))
    }

    //#[post("/chat/<id>/message")]
    //pub fn add_message() {}
}

use std::sync::Mutex;

use uuid::{uuid, Uuid};

use chat::{Chat, Chats, Message, Role};

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
