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

use std::collections::HashMap;
use std::path::{self, Path};
use std::sync::Arc;

use lazy_static::lazy_static;
use mlua::{Lua, LuaSerdeExt, ObjectLike, Table};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use tokio::fs;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{llm::LlmStore, state::StateStore};

mod llm_module;
mod log_module;
mod state_module;

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

        // Add Epibloom modules to the environment.
        {
            let uuid_module = lua.create_table()?;

            uuid_module.set(
                "now_v7",
                lua.create_function(|_lua, ()| {
                    let new_uuid = Uuid::now_v7();
                    Ok(format!("{}", new_uuid.hyphenated()))
                })?,
            )?;

            let package_loaded: Table = globals.get_path("package.loaded")?;
            package_loaded.set("epibloom.uuid", uuid_module)?;
        }

        {
            let time_module = lua.create_table()?;

            time_module.set(
                "now_utc_rfc3339",
                lua.create_function(|_lua, ()| {
                    let now = OffsetDateTime::now_utc();
                    now.format(&Rfc3339).map_err(mlua::Error::external)
                })?,
            )?;

            let package_loaded: Table = globals.get_path("package.loaded")?;
            package_loaded.set("epibloom.time", time_module)?;
        }

        log_module::setup(&lua)?;
        state_module::setup(&lua, state_store)?;
        llm_module::setup(&lua, llm_store)?;

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
