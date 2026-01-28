// TODO: See NOTES.md for ideas like templating, reloading scripts, etc.
//
// Use cases:
// - Press the button to submit a chat message, submit an action in the game, etc. which runs a script that does things like add messages to the chat and starts generation, etc. Needs to be able to return things for the UI to view? Updates the game state which is reflected in the UI?
// - Defining the UI in scripts? As in, behaviour of buttons should be scriptable but should game UI/the API also be as flexible? eg. scripting inventory system instead of having it written in Rust.
//
// Scripts can go in global and local config locations, eg. /etc/epibloom/scripts and ~/.config/epibloom/scripts, and the user one is higher in package.path
//
// There is a POST API endpoint /api/script/<script name> which looks up scripts, passes the body JSON as Lua values to the script, runs it.
// - How do results come back? Via a global event endpoint that streams events? (should this endpoint also just send the current state of the application as the first thing, like chat streams do?) Response has the return values of the script?
// - Each script in its own Lua state or all use the same one? If same one, then can't run scripts in background waiting for things unless using some coroutine async thing - I want to try coroutine and async (mlua has support) where possible.
//
// Where to store game/global state? If same Lua state for all scripts, can be in Lua state and get loaded and saved to/from JSON when needed.
//
// Being able to run multiple scripts in parallel allows for doing multiple unrelated things at once, but eg. game scripts probably should be executed one after the other. Note that some game actions may want to run multiple chats at once, but one script can start multiple chat generations and wait on them all to do what it needs to.
// Perhaps the API should be `/api/script/<script context name>/<script name>` to execute a script in a named Lua state context. The game can default to using either just "game" or an ID for the game session (as in a instantiation of the game, an overall persistent session, including all save files made for that session).
//
// Chat objects in Lua wrap a Uuid and internally the methods call back to the chat store, yielding back where the Rust implementation awaits for the thing to happen.

use std::collections::HashMap;
use std::sync::Arc;

use mlua::{Lua, LuaSerdeExt, ObjectLike, Table};
use tokio::fs;
use tokio::sync::Mutex;

use crate::chat::ChatStore;

pub struct ContextStore {
    contexts: Arc<Mutex<HashMap<String, Arc<Context>>>>,

    /// For scripts to interact with chats.
    chat_store: ChatStore,
}

impl ContextStore {
    pub fn new(chat_store: &ChatStore) -> Self {
        Self {
            contexts: Arc::new(Mutex::new(HashMap::new())),

            // `chat_store.clone()` clones the inner Arc, so this is still a
            // reference to the same chat store.
            chat_store: chat_store.clone(),
        }
    }

    /// Creates the context if it doesn't exist.
    pub async fn get_context(&self, name: &str) -> Result<Arc<Context>, Error> {
        let mut contexts = self.contexts.lock().await;
        let key = name.to_owned();

        match contexts.get(name) {
            Some(context) => Ok(Arc::clone(context)),
            None => {
                let new_context = Arc::new(Context::new(&name)?);
                contexts.insert(key, Arc::clone(&new_context));
                Ok(new_context)
            }
        }
    }
}

/// A script execution context. Script runs on the same context share a
/// persistent Lua state.
pub struct Context {
    name: String,

    // The Tokio mutex unlocks to clients in FIFO order, so callers will
    // automatically implicitly queue for access.
    lua: Mutex<Lua>,
}

impl Context {
    pub fn new(name: &str) -> Result<Self, Error> {
        let lua = Lua::new();

        // TODO: If publishing for general use, sandbox scripts, eg. remove
        // access to io library and provide functions to read and write from the
        // script data directory.

        let globals = lua.globals();

        // Setup `package.path` to include `SCRIPTS_DIR`, so modules can be `require()`d.
        {
            let scripts_dir = crate::SCRIPTS_DIR
                .get()
                .unwrap()
                .to_str()
                .expect("SCRIPTS_DIR was not valid UTF-8");

            let package_config: String = globals.get_path("package.config")?;
            let path_separator = package_config.chars().nth(0).unwrap();
            let template_separator = package_config.chars().nth(1).unwrap();

            let mut package_path: String = globals.get_path("package.path")?;
            package_path.insert_str(
                0,
                &format!(
                    "{scripts_dir}{path_separator}?.lua{template_separator}{scripts_dir}{path_separator}/?/init.lua"
                ),
            );
            let package: Table = globals.get("package")?;
            package.set("path", package_path)?;
        }

        // TODO: Add chat, uuid, logging, etc. libraries

        Ok(Self {
            name: name.to_owned(),
            lua: Mutex::new(lua),
        })
    }

    /// If there is already a script running on this context and/or if there are
    /// some queued up, this script won't run until its spot in the queue comes
    /// up. This means the future won't resolve until then.
    pub async fn run_script(
        &self,
        // TODO: Make this a relative path (example input
        // `game/move_to_location`). If publishing for general use, ensure
        // that the path is relative and doesn't go up and escape the scripts
        // fdirectory.
        script_name: &str,
        input: serde_json::Value,
    ) -> Result<serde_json::Value, Error> {
        info!(
            "Request to run script {script_name} on context {}",
            self.name
        );

        let lua = self.lua.lock().await;

        let input = lua.to_value(&input);

        let mut script_path = crate::SCRIPTS_DIR.get().unwrap().clone();
        script_path.set_file_name(format!("{script_name}.lua"));
        let script_content = fs::read_to_string(&script_path).await?;

        info!("Running script {script_path} on context {}", self.name);

        let result = lua.load(script_content).call_async(input).await?;

        Ok(lua.from_value(result)?)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Lua error: {0}")]
    Lua(#[from] mlua::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
