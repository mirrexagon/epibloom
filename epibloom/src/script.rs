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
use std::path::Path;
use std::sync::Arc;

use futures::FutureExt;
use mlua::{Lua, LuaSerdeExt, ObjectLike, Table, UserData};
use tokio::fs;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::chat::ChatStore;

pub struct ContextStore {
    contexts: Mutex<HashMap<String, Arc<Context>>>,

    /// For scripts to interact with chats.
    chat_store: Arc<ChatStore>,
}

impl ContextStore {
    pub fn new(chat_store: Arc<ChatStore>) -> Self {
        Self {
            contexts: Mutex::new(HashMap::new()),
            chat_store,
        }
    }

    /// Creates the context if it doesn't exist.
    pub async fn get_context(&self, name: &str) -> Result<Arc<Context>, Error> {
        let mut contexts = self.contexts.lock().await;
        let key = name.to_owned();

        match contexts.get(name) {
            Some(context) => Ok(Arc::clone(context)),
            None => {
                let new_context = Arc::new(Context::new(name, Arc::clone(&self.chat_store))?);
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

    /// For scripts to interact with chats.
    chat_store: Arc<ChatStore>,
}

impl Context {
    pub fn new(name: &str, chat_store: Arc<ChatStore>) -> Result<Self, Error> {
        let lua = Lua::new();
        lua.set_named_registry_value("epibloom.context.name", name)?;

        // TODO: If publishing for general use, sandbox scripts, eg. remove
        // access to io library and provide functions to read and write from the
        // script data directory.
        // Also reduce package.path to just Epibloom scripts directory (though scripts can change this, try to avoid that).

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
            let template_separator = package_config.chars().nth(2).unwrap();

            let mut package_path: String = globals.get_path("package.path")?;
            package_path.insert_str(
                0,
                &format!(
                    "{scripts_dir}{path_separator}?.lua{template_separator}{scripts_dir}{path_separator}?{path_separator}init.lua{template_separator}"
                ),
            );
            let package: Table = globals.get("package")?;
            package.set("path", package_path)?;
        }

        // TODO: Add chat, uuid, logging, etc. libraries

        // Logging module.
        {
            let log_module = lua.create_table()?;

            log_module.set(
                "log",
                lua.create_function(|lua, (level, message): (String, String)| {
                    let level = match level.as_str() {
                        "error" => log::Level::Error,
                        "warn" => log::Level::Warn,
                        "info" => log::Level::Info,
                        "debug" => log::Level::Debug,
                        "trace" => log::Level::Trace,
                        _ => {
                            return Err(mlua::Error::RuntimeError(format!(
                                "provided log level {level} is not valid"
                            )))
                        }
                    };

                    let context_name: String = lua.named_registry_value("epibloom.context.name")?;
                    let log_target = format!("script context '{context_name}'");

                    log::log!(target: &log_target, level, "{message}");

                    Ok(())
                })?,
            )?;

            let package_loaded: Table = globals.get_path("package.loaded")?;
            package_loaded.set("epibloom.log", log_module)?;

            lua.load(
                r#"
                    local log = require("epibloom.log")

                    for _, level in ipairs{ "error", "warn", "info", "debug", "trace" } do
                        log[level] = function(message, ...)
                            log.log(level, string.format(tostring(message), ...))
                        end
                    end

                    print = function(...args)
                        for i = 1, args.n do
                            args[i] = tostring(args[i])
                        end

                        log.info(table.concat(args, "\t"))
                    end
                "#,
            )
            .eval::<()>()?;
        }

        // UUID module.
        {
            let uuid_module = lua.create_table()?;

            uuid_module.set(
                "new",
                lua.create_function(|_lua, _: ()| {
                    let new_uuid = Uuid::now_v7();
                    Ok(format!("{}", new_uuid.hyphenated()))
                })?,
            )?;

            let package_loaded: Table = globals.get_path("package.loaded")?;
            package_loaded.set("epibloom.uuid", uuid_module)?;
        }

        // Chat module.
        {
            let chat_module = lua.create_table()?;

            // chat_module.set(
            //     "new",
            //     lua.create_async_function(|lua, name: Option<String>| {
            //         let chat_store = chat_store
            //         self.chat_store
            //     })?,
            // )?;

            let package_loaded: Table = globals.get_path("package.loaded")?;
            package_loaded.set("epibloom.chat", chat_module)?;
        }

        // Filesystem module.
        {
            let filesystem_module = lua.create_table()?;

            // local f = assert(io.open(filesystem.data_file("game1/save1.json"), "w"))
            // -- Opens "~/.local/share/epibloom/data/game1/save1.json"

            let package_loaded: Table = globals.get_path("package.loaded")?;
            package_loaded.set("epibloom.filesystem", filesystem_module)?;
        }

        Ok(Self {
            name: name.to_owned(),
            lua: Mutex::new(lua),
            chat_store: Arc::clone(&chat_store),
        })
    }

    /// If there is already a script running on this context and/or if there are
    /// some queued up, this script won't run until all previously-queued
    /// scripts are run. This means the future won't resolve until then.
    pub async fn run_script<P: AsRef<Path>>(
        &self,
        script_path: P,
        input: serde_json::Value,
    ) -> Result<serde_json::Value, Error> {
        let script_path = script_path.as_ref();

        info!(
            "Request to run script {script_path:?} on context {}",
            self.name
        );

        let lua = self.lua.lock().await;

        let input = lua.to_value(&input);

        let mut filesystem_script_path = crate::SCRIPTS_DIR.get().unwrap().clone();
        filesystem_script_path.push(script_path);
        filesystem_script_path.set_extension("lua");
        filesystem_script_path.canonicalize()?;

        if !filesystem_script_path.starts_with(crate::SCRIPTS_DIR.get().unwrap()) {
            return Err(Error::ScriptPathEscapesScriptDirectory);
        }

        let script_content = fs::read_to_string(&filesystem_script_path).await?;

        info!(
            "Running script {filesystem_script_path:?} on context {}",
            self.name
        );

        let result = lua.load(script_content).call_async(input).await?;

        Ok(lua.from_value(result)?)

        // TODO: Log any errors that happen here, currently they only come back in the HTTP request.
    }
}

struct LuaChatStore {
    chat_store: Arc<ChatStore>,
}

// local all_chats = chats.get_all()
// -- all_chats = { chat_id = chat_object }
//
// local chat = chats.new(name)
// local chat = chats.get(chat_id)
//
// chat.messages = { { role = "user", text = "blah" }. ... }
// -- can't get while generating
// -- get an error when trying to set messages or something in it
//
// chat.push_message(role, text)
//
// chat.generate(backend, backend_config)
// chat.wait_for_generation()

impl UserData for LuaChatStore {
    fn add_fields<F: mlua::UserDataFields<Self>>(fields: &mut F) {}

    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        methods.add_async_method("new", async |lua, cs, name: Option<String>| {
            let new_chat_id = cs
                .chat_store
                .new_chat(name.as_deref())
                .await
                .map_err(|err| mlua::Error::RuntimeError(err.to_string()))?;

            Ok(LuaChat {
                id: new_chat_id,
                chat_store: Arc::clone(&cs.chat_store),
            })
        });
    }
}

struct LuaChat {
    id: Uuid,
    chat_store: Arc<ChatStore>,
}

impl UserData for LuaChat {
    fn add_fields<F: mlua::UserDataFields<Self>>(fields: &mut F) {
        // TODO: messages
    }

    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        // TODO: push_message and generate
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Script path escapes the script directory")]
    ScriptPathEscapesScriptDirectory,

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Lua error: {0}")]
    Lua(#[from] mlua::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
