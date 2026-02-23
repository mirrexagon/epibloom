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

use std::path::{self, Path};
use std::sync::Arc;
use std::{collections::HashMap, path::PathBuf};

use lazy_static::lazy_static;
use mlua::{FromLua, IntoLua, Lua, LuaSerdeExt, ObjectLike, Table, UserData, Value};
use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::sync::{broadcast, Mutex};
use uuid::Uuid;

use crate::state::StateObject;
use crate::trace::TraceEvent;
use crate::{
    backend,
    state::StateStore,
    trace::{self, Message, Role, TraceStore},
};

lazy_static! {
    static ref SERIALIZE_OPTIONS: mlua::SerializeOptions = mlua::SerializeOptions::new()
        .serialize_none_to_null(false)
        .serialize_unit_to_null(false);
    static ref DESERIALIZE_OPTIONS: mlua::DeserializeOptions =
        mlua::DeserializeOptions::new().encode_empty_tables_as_array(true);
}

pub struct ContextStore {
    contexts: Mutex<HashMap<String, Arc<Context>>>,

    /// For scripts to interact with traces.
    trace_store: Arc<TraceStore>,

    /// For scripts to interact with state objects.
    state_store: Arc<StateStore>,
}

impl ContextStore {
    pub fn new(trace_store: Arc<TraceStore>, state_store: Arc<StateStore>) -> Self {
        Self {
            contexts: Mutex::new(HashMap::new()),
            trace_store,
            state_store,
        }
    }

    /// Creates the context if it doesn't exist.
    pub async fn get_context(&self, name: &str) -> Result<Arc<Context>, Error> {
        let mut contexts = self.contexts.lock().await;
        let key = name.to_owned();

        match contexts.get(name) {
            Some(context) => Ok(Arc::clone(context)),
            None => {
                let new_context = Arc::new(Context::new(
                    name,
                    Arc::clone(&self.trace_store),
                    Arc::clone(&self.state_store),
                )?);
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
    pub fn new(
        name: &str,
        trace_store: Arc<TraceStore>,
        state_store: Arc<StateStore>,
    ) -> Result<Self, Error> {
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

        // TODO: Put each module in its own Rust module and expose a function to add it to a Lua state.

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

                        log.log("info", table.concat(args, "\t"))
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
            lua.set_named_registry_value(
                "epibloom.store.trace",
                lua.create_userdata(LuaTraceStore {
                    trace_store: Arc::clone(&trace_store),
                })?,
            )?;

            {
                let trace_store_proxy_module = lua.create_table()?;

                trace_store_proxy_module.set(
                    "new",
                    lua.create_async_function(async |lua, name: Option<String>| {
                        let trace_store: LuaTraceStore =
                            lua.named_registry_value("epibloom.store.trace")?;

                        let new_trace_id = trace_store
                            .trace_store
                            .new_trace(name.as_deref())
                            .await
                            .map_err(mlua::Error::external)?;

                        Ok(LuaTrace {
                            id: new_trace_id,
                            trace_store: Arc::clone(&trace_store.trace_store),
                        })
                    })?,
                )?;

                trace_store_proxy_module.set(
                    "get",
                    lua.create_async_function(async |lua, trace_id: String| {
                        let trace_store: LuaTraceStore =
                            lua.named_registry_value("epibloom.store.trace")?;

                        let trace_id = Uuid::try_parse(&trace_id).map_err(mlua::Error::external)?;

                        if trace_store.trace_store.has_trace(trace_id).await {
                            Ok((
                                Some(LuaTrace {
                                    id: trace_id,
                                    trace_store: Arc::clone(&trace_store.trace_store),
                                }),
                                None,
                            ))
                        } else {
                            Ok((
                                None,
                                Some(format!("trace {} not found", trace_id.hyphenated())),
                            ))
                        }
                    })?,
                )?;

                let package_loaded: Table = globals.get_path("package.loaded")?;
                package_loaded.set("epibloom.trace", trace_store_proxy_module)?;
            }

            {
                let message_metatable = lua.create_table()?;

                message_metatable.set(
                    "__tostring",
                    lua.create_function(|_lua, message: Message| {
                        if message.thinking.is_some() {
                            Ok(format!(
                                "Message {{ role = \"{}\", content = \"{}\", thinking = \"{}\" }}",
                                message.role,
                                message.content,
                                message.thinking.unwrap()
                            ))
                        } else {
                            Ok(format!(
                                "Message {{ role = \"{}\", content = \"{}\" }}",
                                message.role, message.content
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

            filesystem_module.set(
                "files",
                lua.create_async_function(async |_lua, mut pattern: String| {
                    debug!("epibloom.filesystem.files called with pattern '{pattern}'");

                    let script_data_dir = crate::SCRIPT_DATA_DIR.get().unwrap();

                    if !pattern.starts_with('/') {
                        pattern.insert(0, '/');
                    }

                    // Prepend the script data directory.
                    pattern.insert_str(
                        0,
                        script_data_dir.to_str().ok_or(mlua::Error::runtime(
                            "script data directory {script_data_dir:?} is not valid UTF-8",
                        ))?,
                    );

                    debug!("Full pattern is '{pattern}'");

                    // Collect all items in the directory into a vector.
                    let paths = glob::glob(&pattern)
                        .map_err(mlua::Error::external)?
                        .collect::<Result<Vec<_>, glob::GlobError>>()
                        .map_err(mlua::Error::external)?;

                    debug!("Files found: {paths:#?}");

                    // Only keep JSON files, and use the relative path.
                    let paths = paths
                        .iter()
                        .filter_map(|path| {
                            if path.is_file() {
                                Some(path.strip_prefix(script_data_dir).unwrap().to_owned())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>();

                    debug!("Final paths: {paths:#?}");

                    Ok(paths)
                })?,
            )?;

            filesystem_module.set(
                "save_json",
                lua.create_async_function(async |_lua, (file_path, content): (PathBuf, Value)| {
                    let mut path = crate::SCRIPT_DATA_DIR.get().unwrap().clone();
                    path.push(file_path);
                    path = path::absolute(path)?;

                    if !path.starts_with(crate::SCRIPT_DATA_DIR.get().unwrap()) {
                        return Err(mlua::Error::runtime(
                            "path escapes the script data directory",
                        ));
                    }

                    if !path.set_extension("json") {
                        return Err(mlua::Error::runtime("path has no filename part"));
                    }

                    if let Some(parent) = path.parent() {
                        fs::create_dir_all(parent).await?;
                    }

                    let json_str =
                        serde_json::to_string(&content).map_err(mlua::Error::external)?;

                    fs::write(&path, json_str).await?;

                    Ok(())
                })?,
            )?;

            filesystem_module.set(
                "load_json",
                lua.create_async_function(async |lua, file_path: PathBuf| {
                    let mut path = crate::SCRIPT_DATA_DIR.get().unwrap().clone();
                    path.push(file_path);
                    path = path::absolute(path)?;

                    if !path.starts_with(crate::SCRIPT_DATA_DIR.get().unwrap()) {
                        return Err(mlua::Error::runtime(
                            "path {file_path} escapes the script data directory",
                        ));
                    }

                    if !path.set_extension("json") {
                        return Err(mlua::Error::runtime("path has no filename part"));
                    }

                    let json_str =
                        String::from_utf8(fs::read(path).await?).map_err(mlua::Error::external)?;
                    let json_value: serde_json::Value =
                        serde_json::from_str(&json_str).map_err(mlua::Error::external)?;

                    lua.to_value_with(&json_value, *SERIALIZE_OPTIONS)
                })?,
            )?;

            let package_loaded: Table = globals.get_path("package.loaded")?;
            package_loaded.set("epibloom.filesystem", filesystem_module)?;
        }

        // State module.
        {
            lua.set_named_registry_value(
                "epibloom.store.state",
                lua.create_userdata(LuaStateStore {
                    state_store: Arc::clone(&state_store),
                })?,
            )?;

            {
                let state_store_proxy_module = lua.create_table()?;

                state_store_proxy_module.set(
                    "get",
                    lua.create_async_function(async |lua, name: String| {
                        let state_store: LuaStateStore =
                            lua.named_registry_value("epibloom.store.state")?;

                        let state_obj = state_store.state_store.get_or_create(&name).await;

                        Ok(LuaStateObject {
                            name: name.clone(),
                            state_object: state_obj,
                        })
                    })?,
                )?;

                let package_loaded: Table = globals.get_path("package.loaded")?;
                package_loaded.set("epibloom.state", state_store_proxy_module)?;
            }
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

        let input = lua.to_value_with(&input, *SERIALIZE_OPTIONS)?;

        let mut filesystem_script_path = crate::SCRIPTS_DIR.get().unwrap().clone();
        filesystem_script_path.push(script_path);
        filesystem_script_path.set_extension("lua");
        filesystem_script_path = path::absolute(filesystem_script_path)?;

        if !filesystem_script_path.starts_with(crate::SCRIPTS_DIR.get().unwrap()) {
            return Err(Error::ScriptPathEscapesScriptDirectory);
        }

        // TODO: Switch to using anyhow or eyre to add context to errors and
        // make error handling easier in general?
        let script_content = fs::read_to_string(&filesystem_script_path).await?;

        debug!(
            "Running script {filesystem_script_path:?} on context {}",
            self.name
        );

        let result = lua
            .load(script_content)
            .set_name(script_path.to_string_lossy())
            .call_async(input)
            .await
            .inspect_err(|err| {
                log::error!(
                    "Error executing script {script_path:?} on context {}: {err}",
                    self.name,
                )
            })?;

        Ok(lua.from_value_with(result, *DESERIALIZE_OPTIONS)?)
    }
}

struct LuaTraceStore {
    trace_store: Arc<TraceStore>,
}

impl UserData for LuaTraceStore {}

impl FromLua for LuaTraceStore {
    fn from_lua(value: Value, _lua: &Lua) -> mlua::Result<Self> {
        match value {
            Value::UserData(ud) => match ud.borrow::<LuaTraceStore>() {
                Ok(store) => Ok(LuaTraceStore {
                    trace_store: Arc::clone(&store.trace_store),
                }),
                Err(err) => Err(err),
            },
            _ => Err(mlua::Error::runtime(format!(
                "expected userdata, got {value:?}"
            ))),
        }
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
                let chunk_rx = match backend.as_ref() {
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
                            .await?
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
                            .await?
                    }

                    _ => {
                        return Err(mlua::Error::runtime(format!(
                            r#"bad argument #1: unknown backend '{backend}'"#
                        )))
                    }
                };

                Ok(LuaTraceGenerateHandle {
                    receiver: Some(chunk_rx),
                })
            },
        );

        // TODO: Do I need this? Its only use now that I have
        // TraceGenerationHandle is for other script contexts to wait until the
        // trace is free, but I am not planning to reuse traces across scripts.
        methods.add_async_method("wait_for_generate", async |_lua, trace, ()| {
            trace
                .trace_store
                .wait_for_generate_complete(trace.id)
                .await?;

            Ok(())
        });

        methods.add_async_meta_method("__index", async |_lua, trace, message_position: usize| {
            let session = trace
                .trace_store
                .get_session(trace.id)
                .await
                .map_err(mlua::Error::external)?;

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
                    .map_err(mlua::Error::external)?;

                let mut session = session.write().await;

                if message_position == 0 {
                    return Err(mlua::Error::runtime("message position cannot be 0"));
                }

                session
                    .set_message(Some(message_position - 1), message)
                    .await
                    .map_err(mlua::Error::external)?;

                Ok(())
            },
        );

        methods.add_async_meta_method("__len", async |_lua, trace, ()| {
            let session = trace
                .trace_store
                .get_session(trace.id)
                .await
                .map_err(mlua::Error::external)?;

            let session = session.read().await;

            Ok(session.get_message_count())
        });
    }
}

// TODO: use lua.from_value_with(): https://github.com/mlua-rs/mlua/blob/main/examples/serde.rs#L31
// Fields can be made optional by setting #[serde(default)] on the target struct

impl IntoLua for Message {
    fn into_lua(self, lua: &Lua) -> mlua::Result<Value> {
        let message_table = lua.create_table()?;

        message_table.set("role", self.role.into_lua(lua)?)?;
        message_table.set("content", self.content.into_lua(lua)?)?;
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
                content: FromLua::from_lua(table.get("content")?, lua)?,
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
                return Err(mlua::Error::runtime(format!(
                    r#"role must be one of "system", "user", "assistant", got {value:?}"#
                )))
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
        mlua::Error::external(value)
    }
}

/// Handle returned by `trace:generate()`. Supports both chunk-by-chunk
/// iteration (via `handle:chunks()` in a `for` loop) and blocking until
/// generation completes (via `handle:wait()`).
struct LuaTraceGenerateHandle {
    receiver: Option<broadcast::Receiver<TraceEvent>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
enum LuaTraceGenerateChunk {
    Thinking { delta: String },
    Message { delta: String },
    Error { message: String },
}

impl IntoLua for LuaTraceGenerateChunk {
    fn into_lua(self, lua: &Lua) -> mlua::Result<Value> {
        lua.to_value_with(&self, *SERIALIZE_OPTIONS)
    }
}

impl UserData for LuaTraceGenerateHandle {
    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        // __call: receive the next chunk from the generation stream.
        // Returns the chunk string, or nil when generation is complete.
        // Also allows the handle to be directly used in a generic for loop.
        methods.add_async_meta_method_mut(
            "__call",
            async |_lua, mut handle, _args: mlua::MultiValue| match handle.receiver {
                None => Ok(None),
                Some(ref mut rx) => loop {
                    match rx.recv().await {
                        Ok(event) => match event {
                            TraceEvent::AssistantThinkingDelta { delta } => {
                                break Ok(Some(LuaTraceGenerateChunk::Thinking { delta }));
                            }

                            TraceEvent::AssistantMessageDelta { delta } => {
                                break Ok(Some(LuaTraceGenerateChunk::Message { delta }));
                            }
                            TraceEvent::AssistantMessageEnd { position: _ } => {
                                handle.receiver.take();
                                break Ok(None);
                            }
                            TraceEvent::Error { message } => {
                                handle.receiver.take();
                                break Ok(Some(LuaTraceGenerateChunk::Error { message }));
                            }
                            _ => {}
                        },

                        Err(err) => {
                            handle.receiver.take();
                            break Ok(Some(LuaTraceGenerateChunk::Error {
                                message: err.to_string(),
                            }));
                        }
                    }
                },
            },
        );

        // wait(): block until generation completes, discarding all chunks.
        // Safe to call after partial iteration of chunks() as it drains whatever
        // remains. Safe to call after generation is already done as it returns
        // immediately since the receiver has already been disposed of.
        methods.add_async_method_mut("wait", async |_lua, mut handle, ()| match handle.receiver {
            None => Ok((true, None)),
            Some(ref mut rx) => loop {
                match rx.recv().await {
                    Ok(event) => match event {
                        TraceEvent::AssistantMessageEnd { position: _ } => {
                            handle.receiver.take();
                            break Ok((true, None));
                        }
                        TraceEvent::Error { message } => {
                            handle.receiver.take();
                            break Ok((false, Some(message)));
                        }
                        _ => {}
                    },

                    Err(err) => {
                        handle.receiver.take();
                        break Ok((false, Some(err.to_string())));
                    }
                }
            },
        });
    }
}

struct LuaStateStore {
    state_store: Arc<StateStore>,
}

impl UserData for LuaStateStore {}

impl FromLua for LuaStateStore {
    fn from_lua(value: Value, _lua: &Lua) -> mlua::Result<Self> {
        match value {
            Value::UserData(ud) => match ud.borrow::<LuaStateStore>() {
                Ok(store) => Ok(LuaStateStore {
                    state_store: Arc::clone(&store.state_store),
                }),
                Err(err) => Err(err),
            },
            _ => Err(mlua::Error::runtime(format!(
                "expected userdata, got {value:?}"
            ))),
        }
    }
}

struct LuaStateObject {
    name: String,
    state_object: Arc<StateObject>,
}

impl UserData for LuaStateObject {
    fn add_fields<F: mlua::UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("name", |_lua, state| Ok(state.name.clone()));
    }

    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        // Read current value
        methods.add_async_method("read", |lua, obj, ()| async move {
            let value = obj.state_object.read().await;
            let lua_value = lua.to_value_with(&value, *SERIALIZE_OPTIONS)?;
            Ok(lua_value)
        });

        // Atomic read-modify-write helper
        methods.add_async_method("update", |lua, obj, func: mlua::Function| async move {
            obj.state_object
                .update(async |old_value| {
                    let lua_old_value = lua.to_value_with(&old_value, *SERIALIZE_OPTIONS)?;
                    let lua_new_value: Value = func.call_async(lua_old_value).await?;
                    lua.from_value_with(lua_new_value, *DESERIALIZE_OPTIONS)
                })
                .await
        });
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
