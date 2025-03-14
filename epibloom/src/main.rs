#[macro_use]
extern crate rocket;

mod chat;

mod api {
    use std::sync::Mutex;

    use rocket::serde::{Deserialize, Serialize};
    use rocket::{get, http::Status, response::status, serde::json::Json, State};
    use time::macros::format_description;
    use time::OffsetDateTime;
    use uuid::Uuid;

    use crate::chat::{Chat, Chats};

    #[derive(Debug, Clone, Copy, Serialize, Deserialize)]
    #[serde(crate = "rocket::serde")]
    pub struct NewChatResponse {
        id: Uuid,
    }

    #[post("/chat")]
    pub fn new_chat(
        chats: &State<Mutex<Chats>>,
    ) -> Result<Json<NewChatResponse>, status::Custom<()>> {
        let mut chats = chats.lock().unwrap();

        let new_chat_id = Uuid::new_v4();
        if chats.0.contains_key(&new_chat_id) {
            return Err(status::Custom(Status::InternalServerError, ()));
        }

        let current_datetime = OffsetDateTime::now_local().unwrap();
        let dtfmt = format_description!(version = 2, "[year]-[month]-[day]T[hour][minute][second]");
        let new_chat_name = current_datetime.format(&dtfmt).unwrap();
        chats.0.insert(new_chat_id, Chat::new(&new_chat_name));

        Ok(Json(NewChatResponse { id: new_chat_id }))
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

    //#[post("/chat/<id>/message")]
    //pub fn add_message() {}
}

use std::sync::Mutex;

use uuid::{uuid, Uuid};

use chat::{Chat, Chats, Message, Role};

#[launch]
fn rocket() -> _ {
    let chats = Chats::new();
    let chats = Mutex::new(chats);

    rocket::build()
        .mount(
            "/api",
            routes![api::get_chats, api::get_chat_by_id, api::new_chat],
        )
        .manage(chats)
}
