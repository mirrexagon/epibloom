#[macro_use]
extern crate rocket;

mod chat;

mod api {
    use std::sync::Mutex;

    use rocket::{get, response::status, serde::json::Json, State};
    use uuid::Uuid;

    use crate::chat::{Chat, Chats};

    #[get("/chat/<id>")]
    pub fn get_chat(
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
    let mut chats = Chats::new();

    let test_chat_id = uuid!("165caddf-d42b-4fef-9616-a2e64a370fc9");

    chats.0.insert(
        test_chat_id,
        Chat {
            system_prompt: "You are a helpful assistant.".into(),
            messages: vec![Message {
                role: Role::User,
                message: "Give me a car".into(),
            }],
        },
    );

    let chats = Mutex::new(chats);

    rocket::build()
        .mount("/api", routes![api::get_chat])
        .manage(chats)
}
