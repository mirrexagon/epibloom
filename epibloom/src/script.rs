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
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;

use crate::state::StateObject;
use crate::{
    backend::anthropic::{self, messages::{request, ContentBlock, CacheControl}},
    llm::{LlmChunk, LlmStore, TraceMessage},
    state::StateStore,
};

lazy_static! {
    static ref SERIALIZE_OPTIONS: mlua::SerializeOptions = mlua::SerializeOptions::new()
        .serialize_none_to_null(false)
        .serialize_unit_to_null(false);
    static ref DESERIALIZE_OPTIONS: mlua::DeserializeOptions = mlua::DeserializeOptions::new()
        .encode_empty_tables_as_array(true)
        .detect_mixed_tables(true);
}

pub struct ContextStore {
    contexts: Mutex<HashMap<String, Arc<Context>>>,

    /// For scripts to call LLM generation.
    llm_store: Arc<LlmStore>,

    /// For scripts to interact with state objects.
    state_store: Arc<StateStore>,
}

impl ContextStore {
    pub fn new(llm_store: Arc<LlmStore>, state_store: Arc<StateStore>) -> Self {
        Self {
            contexts: Mutex::new(HashMap::new()),
            llm_store,
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
                    Arc::clone(&self.llm_store),
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
        llm_store: Arc<LlmStore>,
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

        // LLM module.
        {
            lua.set_named_registry_value(
                "epibloom.llm.store",
                lua.create_userdata(LuaLlmStore {
                    llm_store: Arc::clone(&llm_store),
                })?,
            )?;

            let llm_module = lua.create_table()?;

            llm_module.set(
                "generate",
                lua.create_async_function(
                    async |lua, (messages_table, backend, config): (mlua::Table, String, Value)| {
                        if backend != "anthropic" {
                            return Err(mlua::Error::runtime(format!(
                                "unsupported backend '{backend}'; only 'anthropic' is supported"
                            )));
                        }

                        let lua_messages: Vec<LuaLlmMessage> = messages_table
                            .sequence_values::<LuaLlmMessage>()
                            .collect::<mlua::Result<_>>()?;

                        let cfg: anthropic::Config = match config {
                            Value::Nil => anthropic::Config::default(),
                            v => lua.from_value_with(v, *DESERIALIZE_OPTIONS)?,
                        };

                        let store: LuaLlmStore =
                            lua.named_registry_value("epibloom.llm.store")?;

                        let (trace_messages, system, anthropic_messages) =
                            lua_messages_to_llm_input(lua_messages);

                        let (trace_id, rx) = store
                            .llm_store
                            .generate(trace_messages, system, anthropic_messages, cfg)
                            .await
                            .map_err(mlua::Error::external)?;

                        Ok(LuaLlmGenerateHandle {
                            trace_id,
                            receiver: Some(rx),
                        })
                    },
                )?,
            )?;

            let package_loaded: Table = globals.get_path("package.loaded")?;
            package_loaded.set("epibloom.llm", llm_module)?;
        }

        // Filesystem module.
        {
            let filesystem_module = lua.create_table()?;

            filesystem_module.set(
                "files",
                lua.create_async_function(async |_lua, mut pattern: String| {
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

                    // Collect all items in the directory into a vector.
                    let paths = glob::glob(&pattern)
                        .map_err(mlua::Error::external)?
                        .collect::<Result<Vec<_>, glob::GlobError>>()
                        .map_err(mlua::Error::external)?;

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

// ---------------------------------------------------------------------------
// LLM Lua bindings
// ---------------------------------------------------------------------------

struct LuaLlmStore {
    llm_store: Arc<LlmStore>,
}

impl UserData for LuaLlmStore {}

impl FromLua for LuaLlmStore {
    fn from_lua(value: Value, _lua: &Lua) -> mlua::Result<Self> {
        match value {
            Value::UserData(ud) => match ud.borrow::<LuaLlmStore>() {
                Ok(store) => Ok(LuaLlmStore {
                    llm_store: Arc::clone(&store.llm_store),
                }),
                Err(err) => Err(err),
            },
            _ => Err(mlua::Error::runtime(format!(
                "expected userdata, got {value:?}"
            ))),
        }
    }
}

/// Role of a message author (for Lua message parsing).
#[derive(Debug, PartialEq, Clone, Copy)]
enum Role {
    System,
    User,
    Assistant,
}

impl FromLua for Role {
    fn from_lua(value: Value, _lua: &Lua) -> mlua::Result<Self> {
        match value.as_string() {
            Some(role) => match role.to_string_lossy().as_str() {
                "system" => Ok(Role::System),
                "user" => Ok(Role::User),
                "assistant" => Ok(Role::Assistant),
                v => Err(mlua::Error::runtime(format!("unknown role: {v}"))),
            },
            None => Err(mlua::Error::runtime(format!(
                r#"role must be one of "system", "user", "assistant", got {value:?}"#
            ))),
        }
    }
}

/// Lua-facing LLM message. `content` may be a plain string or a list
/// of text content blocks (which may carry `cache_control`).
struct LuaLlmMessage {
    role: Role,
    /// Plain string shorthand, or a list of text content blocks.
    content: LuaLlmContent,
    /// Optional thinking text (assistant turns with extended thinking).
    thinking: Option<String>,
}

enum LuaLlmContent {
    Plain(String),
    Blocks(Vec<LuaTextBlock>),
}

struct LuaTextBlock {
    text: String,
    cache_control: Option<LuaCacheControl>,
}

enum LuaCacheControl {
    Ephemeral,
}

impl FromLua for LuaLlmMessage {
    fn from_lua(value: Value, lua: &Lua) -> mlua::Result<Self> {
        let table = match value {
            Value::Table(t) => t,
            _ => return Err(mlua::Error::runtime("message must be a table")),
        };

        let role: Role = FromLua::from_lua(table.get("role")?, lua)?;
        let thinking: Option<String> = table.get("thinking")?;

        let content_value: Value = table.get("content")?;
        let content = match content_value {
            Value::String(s) => LuaLlmContent::Plain(s.to_string_lossy().to_owned()),
            Value::Table(t) => {
                let blocks: Vec<LuaTextBlock> = t
                    .sequence_values::<LuaTextBlock>()
                    .collect::<mlua::Result<_>>()?;
                LuaLlmContent::Blocks(blocks)
            }
            Value::Nil => LuaLlmContent::Plain(String::new()),
            other => {
                return Err(mlua::Error::runtime(format!(
                    "message content must be a string or table of content blocks, got {other:?}"
                )))
            }
        };

        Ok(LuaLlmMessage { role, content, thinking })
    }
}

impl FromLua for LuaTextBlock {
    fn from_lua(value: Value, _lua: &Lua) -> mlua::Result<Self> {
        let table = match value {
            Value::Table(t) => t,
            _ => return Err(mlua::Error::runtime("text block must be a table")),
        };

        let text: String = table.get("text")?;
        let cache_control: Option<LuaCacheControl> = match table.get("cache_control")? {
            Value::Nil => None,
            Value::Table(cc) => {
                let cc_type: String = cc.get("type")?;
                match cc_type.as_str() {
                    "ephemeral" => Some(LuaCacheControl::Ephemeral),
                    other => return Err(mlua::Error::runtime(format!(
                        "unknown cache_control type: {other}"
                    ))),
                }
            }
            other => return Err(mlua::Error::runtime(format!(
                "cache_control must be a table, got {other:?}"
            ))),
        };

        Ok(LuaTextBlock { text, cache_control })
    }
}

/// Convert Lua LLM messages into:
/// 1. `Vec<TraceMessage>` — plain-text messages stored in the trace state object.
/// 2. `Option<String>` — system prompt extracted from any system-role messages.
/// 3. `Vec<request::Message>` — full Anthropic API messages with content blocks.
fn lua_messages_to_llm_input(
    lua_messages: Vec<LuaLlmMessage>,
) -> (Vec<TraceMessage>, Option<String>, Vec<request::Message>) {
    let mut trace_messages: Vec<TraceMessage> = Vec::new();
    let mut system_parts: Vec<String> = Vec::new();
    let mut anthropic_messages: Vec<request::Message> = Vec::new();

    for msg in lua_messages {
        // Flatten content blocks to plain text for trace storage.
        let plain_text = match &msg.content {
            LuaLlmContent::Plain(s) => s.clone(),
            LuaLlmContent::Blocks(blocks) => {
                blocks.iter().map(|b| b.text.as_str()).collect::<Vec<_>>().join("")
            }
        };

        if msg.role == Role::System {
            // System messages are stored in the trace for visibility but go
            // into the top-level `system` field for the Anthropic API.
            trace_messages.push(TraceMessage {
                role: "system".to_owned(),
                content: plain_text.clone(),
                thinking: None,
            });
            system_parts.push(plain_text);
            continue;
        }

        // Build the Anthropic content block list.
        let anthropic_content: Vec<ContentBlock> = match msg.content {
            LuaLlmContent::Plain(text) => vec![ContentBlock::Text {
                text,
                citations: None,
                cache_control: None,
            }],
            LuaLlmContent::Blocks(blocks) => blocks
                .into_iter()
                .map(|b| ContentBlock::Text {
                    text: b.text,
                    citations: None,
                    cache_control: b.cache_control.map(|cc| match cc {
                        LuaCacheControl::Ephemeral => CacheControl::Ephemeral,
                    }),
                })
                .collect(),
        };

        let anthropic_role = match msg.role {
            Role::User => crate::backend::anthropic::messages::Role::User,
            Role::Assistant => crate::backend::anthropic::messages::Role::Assistant,
            Role::System => unreachable!("system messages already handled above"),
        };

        trace_messages.push(TraceMessage {
            role: match msg.role {
                Role::User => "user".to_owned(),
                Role::Assistant => "assistant".to_owned(),
                Role::System => unreachable!(),
            },
            content: plain_text,
            thinking: msg.thinking,
        });

        anthropic_messages.push(request::Message {
            role: anthropic_role,
            content: anthropic_content,
        });
    }

    let system = if system_parts.is_empty() {
        None
    } else {
        Some(system_parts.join("\n\n"))
    };

    (trace_messages, system, anthropic_messages)
}

/// Handle returned by `llm.generate()`. Supports chunk-by-chunk iteration
/// (via `__call`) and blocking until complete (via `wait()`).
struct LuaLlmGenerateHandle {
    trace_id: String,
    receiver: Option<mpsc::Receiver<LlmChunk>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
enum LuaLlmGenerateChunk {
    Thinking { delta: String },
    Message { delta: String },
    Error { message: String },
}

impl IntoLua for LuaLlmGenerateChunk {
    fn into_lua(self, lua: &Lua) -> mlua::Result<Value> {
        lua.to_value_with(&self, *SERIALIZE_OPTIONS)
    }
}

impl UserData for LuaLlmGenerateHandle {
    fn add_fields<F: mlua::UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("trace_id", |_lua, handle| Ok(handle.trace_id.clone()));
    }

    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        // __call: return the next chunk, or nil when generation is complete.
        methods.add_async_meta_method_mut(
            "__call",
            async |_lua, mut handle, _args: mlua::MultiValue| {
                // Receive one chunk. The borrow of `handle.receiver` via
                // `ref mut rx` ends before we call `.take()` below.
                let chunk = match handle.receiver {
                    None => return Ok(None),
                    Some(ref mut rx) => rx.recv().await,
                };
                match chunk {
                    Some(LlmChunk::Thinking { delta }) => {
                        Ok(Some(LuaLlmGenerateChunk::Thinking { delta }))
                    }
                    Some(LlmChunk::Message { delta }) => {
                        Ok(Some(LuaLlmGenerateChunk::Message { delta }))
                    }
                    Some(LlmChunk::Error { message }) => {
                        handle.receiver.take();
                        Ok(Some(LuaLlmGenerateChunk::Error { message }))
                    }
                    None => {
                        handle.receiver.take();
                        Ok(None)
                    }
                }
            },
        );

        // wait(): drain remaining chunks, returning (success, error_message).
        methods.add_async_method_mut("wait", async |_lua, mut handle, ()| loop {
            let chunk = match handle.receiver {
                None => return Ok((true, None)),
                Some(ref mut rx) => rx.recv().await,
            };
            match chunk {
                None => {
                    handle.receiver.take();
                    return Ok((true, None));
                }
                Some(LlmChunk::Error { message }) => {
                    handle.receiver.take();
                    return Ok((false, Some(message)));
                }
                Some(_) => {} // discard thinking/message chunks
            }
        });
    }
}

// ---------------------------------------------------------------------------
// State object Lua bindings
// ---------------------------------------------------------------------------

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
                    let value = lua.from_value_with(lua_new_value, *DESERIALIZE_OPTIONS)?;
                    Ok(value)
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
