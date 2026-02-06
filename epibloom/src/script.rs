// TODO: See NOTES.md for ideas like templating, reloading scripts, etc.
//
// Use cases:
// - Press the button to submit a chat message, submit an action in the game, etc. which runs a script that does things like add messages to the chat and starts generation, etc. Needs to be able to return things for the UI to view? Updates the game state which is reflected in the UI?
// - Defining the UI in scripts? As in, behaviour of buttons should be scriptable but should game UI/the API also be as flexible? eg. scripting inventory system instead of having it written in Rust.
//
// Scripts can go in global and local config locations, eg. /etc/epibloom/scripts and ~/.config/epibloom/scripts, and the user one is higher in package.path
//
// Tool calls: Lua should be able to attach tool call possibilities with Lua
// functions that call back into the state. But I also want to be able to run
// multiple generations at once.
//
// Be able to send responses to the client in the middle of a script, via SSE.
// eg. for letting the UI know to change button states, etc.?

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use mlua::{FromLua, IntoLua, Lua, LuaSerdeExt, ObjectLike, Table, UserData, Value};
use tokio::fs;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{
    backend,
    trace::{self, Message, Role, Trace, TraceStore},
};

pub struct ContextStore {
    contexts: Mutex<HashMap<String, Arc<Context>>>,

    /// For scripts to interact with traces.
    trace_store: Arc<TraceStore>,
}

impl ContextStore {
    pub fn new(trace_store: Arc<TraceStore>) -> Self {
        Self {
            contexts: Mutex::new(HashMap::new()),
            trace_store,
        }
    }

    /// Creates the context if it doesn't exist.
    pub async fn get_context(&self, name: &str) -> Result<Arc<Context>, Error> {
        let mut contexts = self.contexts.lock().await;
        let key = name.to_owned();

        match contexts.get(name) {
            Some(context) => Ok(Arc::clone(context)),
            None => {
                let new_context = Arc::new(Context::new(name, Arc::clone(&self.trace_store))?);
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
    pub fn new(name: &str, trace_store: Arc<TraceStore>) -> Result<Self, Error> {
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
                            return Err(mlua::Error::runtime(format!(
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
            .set_name("epibloom.log module setup")
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

        // Trace module.
        {
            let traces = lua.create_userdata(LuaTraceStore {
                trace_store: Arc::clone(&trace_store),
            })?;

            let package_loaded: Table = globals.get_path("package.loaded")?;
            package_loaded.set("epibloom.trace", traces)?;

            {
                let message_metatable = lua.create_table()?;

                message_metatable.set(
                    "__tostring",
                    lua.create_function(|_lua, message: Message| {
                        if message.thinking.is_some() {
                            Ok(format!(
                                "Message {{ role = \"{}\", text = \"{}\", thinking = \"{}\" }}",
                                message.role,
                                message.text,
                                message.thinking.unwrap()
                            ))
                        } else {
                            Ok(format!(
                                "Message {{ role = \"{}\", text = \"{}\" }}",
                                message.role, message.text
                            ))
                        }
                    })?,
                )?;

                lua.set_named_registry_value(
                    "epibloom.trace.message.metatable",
                    message_metatable,
                )?;
            }
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

struct LuaTraceStore {
    trace_store: Arc<TraceStore>,
}

impl UserData for LuaTraceStore {
    fn add_fields<F: mlua::UserDataFields<Self>>(fields: &mut F) {}

    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        methods.add_async_method("new", async |_lua, cs, name: Option<String>| {
            let new_trace_id = cs
                .trace_store
                .new_trace(name.as_deref())
                .await
                .map_err(|err| mlua::Error::runtime(err.to_string()))?;

            Ok(LuaTrace {
                id: new_trace_id,
                trace_store: Arc::clone(&cs.trace_store),
            })
        });

        // TODO: Get existing chat
    }
}

struct LuaTrace {
    id: Uuid,
    trace_store: Arc<TraceStore>,
}

impl UserData for LuaTrace {
    fn add_fields<F: mlua::UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("id", |_lua, trace| Ok(format!("{}", trace.id.hyphenated())));
    }

    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        methods.add_async_method(
            "generate",
            async |lua, trace, (backend, config): (String, Option<Table>)| {
                match backend.as_ref() {
                    // TODO: Reduce this duplication.
                    "anthropic" => {
                        let config = match config {
                            Some(config_table) => backend::anthropic::Config::from_lua(
                                Value::Table(config_table),
                                &lua,
                            )?,
                            None => backend::anthropic::Config::default(),
                        };

                        trace
                            .trace_store
                            .start_generating::<backend::anthropic::AnthropicApi>(trace.id, config)
                            .await?;
                    }

                    "featherless" => {
                        let config = match config {
                            Some(config_table) => backend::featherless::Config::from_lua(
                                Value::Table(config_table),
                                &lua,
                            )?,
                            None => backend::featherless::Config::default(),
                        };

                        trace
                            .trace_store
                            .start_generating::<backend::featherless::FeatherlessApi>(
                                trace.id, config,
                            )
                            .await?;
                    }

                    _ => {
                        return Err(mlua::Error::runtime(format!(
                            r#"bad argument #1: unknown backend '{backend}'"#
                        )))
                    }
                }

                // TODO: Return something that can be waited on for the generation to complete.

                Ok(())
            },
        );

        methods.add_async_meta_method("__index", async |_lua, trace, message_position: usize| {
            let session = trace
                .trace_store
                .get_session(trace.id)
                .await
                .map_err(mlua::Error::runtime)?;

            let session = session.read().await;

            if message_position == 0 {
                return Err(mlua::Error::runtime("message position cannot be 0"));
            }

            Ok(session.get_message(message_position - 1))
        });

        methods.add_async_meta_method(
            "__newindex",
            async |_lua, trace, (message_position, message): (usize, Message)| {
                let session = trace
                    .trace_store
                    .get_session(trace.id)
                    .await
                    .map_err(mlua::Error::runtime)?;

                let mut session = session.write().await;

                if message_position == 0 {
                    return Err(mlua::Error::runtime("message position cannot be 0"));
                }

                session
                    .set_message(Some(message_position - 1), message)
                    .await
                    .map_err(mlua::Error::runtime)?;

                Ok(())
            },
        );
    }
}

impl IntoLua for Message {
    fn into_lua(self, lua: &Lua) -> mlua::Result<Value> {
        let message_table = lua.create_table()?;

        message_table.set("role", self.role.into_lua(lua)?)?;
        message_table.set("text", self.text.into_lua(lua)?)?;
        message_table.set("thinking", self.thinking.into_lua(lua)?)?;

        let metatable = lua.named_registry_value("epibloom.trace.message.metatable")?;
        message_table.set_metatable(Some(metatable))?;

        Ok(Value::Table(message_table))
    }
}

impl IntoLua for Role {
    fn into_lua(self, lua: &Lua) -> mlua::Result<Value> {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
        }
        .into_lua(lua)
    }
}

impl FromLua for Message {
    fn from_lua(value: Value, lua: &Lua) -> mlua::Result<Self> {
        match value {
            Value::Table(table) => Ok(Message {
                role: FromLua::from_lua(table.get("role")?, lua)?,
                text: FromLua::from_lua(table.get("text")?, lua)?,
                thinking: FromLua::from_lua(table.get("thinking")?, lua)?,
            }),
            _ => Err(mlua::Error::runtime("message must be a table")),
        }
    }
}

impl FromLua for Role {
    fn from_lua(value: Value, _lua: &Lua) -> mlua::Result<Self> {
        let role = match value.as_string() {
            Some(role) => match role.to_string_lossy().as_str() {
                "system" => Role::System,
                "user" => Role::User,
                "assistant" => Role::Assistant,
                value => return Err(mlua::Error::runtime(format!("unknown role: {value}"))),
            },
            None => {
                return Err(mlua::Error::runtime(
                    r#"role must be one of "system", "user", "assistant", got {value:?}"#,
                ))
            }
        };

        Ok(role)
    }
}

impl FromLua for backend::anthropic::Config {
    fn from_lua(value: Value, lua: &Lua) -> mlua::Result<Self> {
        match value {
            Value::Table(config_table) => {
                let mut config = Self::default();

                // TODO: Reduce this duplication.
                let max_tokens: Option<u32> =
                    FromLua::from_lua(config_table.get::<Value>("max_tokens")?, lua)?;
                if let Some(max_tokens) = max_tokens {
                    config.max_tokens = max_tokens;
                }

                let model: Option<String> =
                    FromLua::from_lua(config_table.get::<Value>("model")?, lua)?;
                if let Some(model) = model {
                    config.model = model;
                }

                let stop_sequences: Option<Vec<String>> =
                    FromLua::from_lua(config_table.get::<Value>("stop_sequences")?, lua)?;
                if let Some(stop_sequences) = stop_sequences {
                    config.stop_sequences = stop_sequences;
                }

                let thinking_budget: Option<u32> =
                    FromLua::from_lua(config_table.get::<Value>("thinking_budget")?, lua)?;
                config.thinking_budget = thinking_budget;

                Ok(config)
            }

            value => Err(mlua::Error::runtime(format!(
                "Anthropic config should be a table, got {:#?}",
                value
            ))),
        }
    }
}

impl FromLua for backend::featherless::Config {
    fn from_lua(value: Value, lua: &Lua) -> mlua::Result<Self> {
        match value {
            Value::Table(config_table) => {
                let mut config = Self::default();

                let max_tokens: Option<u32> =
                    FromLua::from_lua(config_table.get::<Value>("max_tokens")?, lua)?;
                if let Some(max_tokens) = max_tokens {
                    config.max_tokens = max_tokens;
                }

                let model: Option<String> =
                    FromLua::from_lua(config_table.get::<Value>("model")?, lua)?;
                if let Some(model) = model {
                    config.model = model;
                }

                let temperature: Option<f32> =
                    FromLua::from_lua(config_table.get::<Value>("temperature")?, lua)?;
                if temperature.is_some() {
                    config.temperature = temperature;
                }

                let top_p: Option<f32> =
                    FromLua::from_lua(config_table.get::<Value>("top_p")?, lua)?;
                if top_p.is_some() {
                    config.top_p = top_p;
                }

                let top_k: Option<i32> =
                    FromLua::from_lua(config_table.get::<Value>("top_k")?, lua)?;
                if top_k.is_some() {
                    config.top_k = top_k;
                }

                let stop_sequences: Option<Vec<String>> =
                    FromLua::from_lua(config_table.get::<Value>("stop_sequences")?, lua)?;
                if let Some(stop_sequences) = stop_sequences {
                    config.stop_sequences = stop_sequences;
                }

                Ok(config)
            }

            value => Err(mlua::Error::runtime(format!(
                "Featherless config should be a table, got {:#?}",
                value
            ))),
        }
    }
}

impl From<trace::Error> for mlua::Error {
    fn from(value: trace::Error) -> Self {
        mlua::Error::runtime(value.to_string())
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
