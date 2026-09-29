// TODO: LLM tool calls: Lua should be able to attach tool call possibilities
// with Lua functions that call back into the state. But I also want to be able
// to run multiple generations at once.

use std::collections::HashMap;
use std::path::{self, Path, PathBuf};
use std::sync::Arc;
use std::thread;

use lazy_static::lazy_static;
use mlua::{Lua, LuaSerdeExt, ObjectLike, Table};
use serde::Serialize;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use tokio::fs;
use tokio::runtime;
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;

use crate::state::StateObject;
use crate::{llm::LlmStore, state::StateStore};

mod llm_module;
mod log_module;
mod state_module;

const SCRIPT_STATUS_HISTORY_OBJECT_NAME_PREFIX: &str = "script-status:";

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

    script_request_sender: mpsc::Sender<ScriptRunRequest>,
    /// Each context runs on its own dedicated OS thread (not a Tokio worker),
    /// since Lua code can block (eg. `os.execute("sleep 1")`, busy loops),
    /// and blocking a Tokio worker thread can starve tasks spawned from it.
    /// In particular, a task spawned from a worker goes into that worker's
    /// LIFO slot, which other workers can't steal from, so eg. an LLM
    /// generation task wouldn't start until the script yields or finishes.
    thread: thread::JoinHandle<()>,
    status_history: Arc<Mutex<ScriptStatusHistory>>,
}

struct ScriptStatusHistory {
    context_name: String,
    status_history: Vec<ScriptStatus>,

    state_store: Arc<StateStore>,
    state_object: Option<Arc<StateObject>>,
}

impl ScriptStatusHistory {
    async fn sync_to_state_object(&mut self) {
        let has_state_object = self.state_object.is_some();
        if !has_state_object {
            self.state_object = Some(
                self.state_store
                    .get_or_create(&format!(
                        "{SCRIPT_STATUS_HISTORY_OBJECT_NAME_PREFIX}{}",
                        self.context_name
                    ))
                    .await
                    .expect("Couldn't create script context status object"),
            );
        }

        if let Err(err) = self
            .state_object
            .as_ref()
            .unwrap()
            .update(async |_| serde_json::to_value(&self.status_history))
            .await
        {
            log::warn!(
                "Failed to sync script status to state object {}: {err}",
                &self.state_object.as_ref().unwrap().name
            );
        }
    }
}

#[derive(Debug, Serialize)]
struct ScriptStatus {
    script_path: String,

    #[serde(with = "time::serde::rfc3339")]
    request_time: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    start_time: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    end_time: Option<OffsetDateTime>,

    input: serde_json::Value,
    result: serde_json::Value,

    error: Option<String>,
}

struct ScriptRunRequest {
    script_path: PathBuf,
    input: serde_json::Value,
    index: usize,
}

impl ScriptRunRequest {
    fn new<P: AsRef<Path>>(script_path: P, input: serde_json::Value, index: usize) -> Self {
        Self {
            script_path: script_path.as_ref().to_owned(),
            input,
            index,
        }
    }
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

            // Asynchronously sleep for the given number of seconds (can be
            // fractional). Unlike eg. `os.execute("sleep 1")`, this doesn't
            // block the context's thread.
            time_module.set(
                "sleep",
                lua.create_async_function(async |_lua, seconds: f64| {
                    let duration =
                        std::time::Duration::try_from_secs_f64(seconds).map_err(|e| {
                            mlua::Error::runtime(format!("invalid sleep duration {seconds}: {e}"))
                        })?;
                    tokio::time::sleep(duration).await;
                    Ok(())
                })?,
            )?;

            let package_loaded: Table = globals.get_path("package.loaded")?;
            package_loaded.set("epibloom.time", time_module)?;
        }

        log_module::setup(&lua)?;
        state_module::setup(&lua, Arc::clone(&state_store))?;
        llm_module::setup(&lua, llm_store)?;

        // Set up script context task.
        let (script_request_sender, mut script_request_receiver) =
            mpsc::channel::<ScriptRunRequest>(64);
        let context_name = name.to_owned();

        let status_history = Arc::new(Mutex::new(ScriptStatusHistory {
            context_name: name.to_owned(),
            status_history: Vec::new(),
            state_store: Arc::clone(&state_store),
            state_object: None,
        }));
        let task_status_history = Arc::clone(&status_history);

        // `Handle::block_on` runs the future on this dedicated thread, while
        // IO/timers are still driven by the main runtime, and `tokio::spawn`
        // inside it spawns onto the main runtime's workers (via the injection
        // queue, which all workers can take from).
        let runtime_handle = runtime::Handle::current();
        let thread = thread::Builder::new()
            .name(format!("script-context-{name}"))
            .spawn(move || runtime_handle.block_on(async move {
            while let Some(request) = script_request_receiver.recv().await {
                info!(
                    "Request to run script {} on context {}",
                    request.script_path.display(),
                    context_name
                );

                macro_rules! update_status {
                    ($update_function:expr) => {{
                        let mut status_history = task_status_history.lock().await;
                        let status = &mut status_history.status_history[request.index];
                        ($update_function)(status);
                        status_history.sync_to_state_object().await;
                    }};
                }

                macro_rules! report_error {
                    ($message:expr) => {{
                        let msg = $message;
                        log::warn!("{}", &msg);
                        update_status!(|status: &mut ScriptStatus| {
                            status.end_time = Some(OffsetDateTime::now_utc());
                            status.error = Some(msg.to_owned());
                        });
                    }};
                }

                update_status!(|status: &mut ScriptStatus| {
                    status.start_time = Some(OffsetDateTime::now_utc());
                });

                let input = match lua.to_value_with(&request.input, *SERIALIZE_OPTIONS) {
                    Ok(v) => v,
                    Err(e) => {
                        report_error!(&format!("Failed to convert input to Lua value: {e}"));
                        continue;
                    }
                };

                let mut filesystem_script_path = crate::SCRIPTS_DIR.get().unwrap().clone();
                filesystem_script_path.push(&request.script_path);
                filesystem_script_path.set_extension("lua");
                filesystem_script_path = match path::absolute(filesystem_script_path) {
                    Ok(v) => v,
                    Err(e) => {
                        report_error!(&format!(
                            "Failed to get filesystem path for script {:?}: {e}",
                            request.script_path
                        ));
                        continue;
                    }
                };

                if !filesystem_script_path.starts_with(crate::SCRIPTS_DIR.get().unwrap()) {
                    report_error!(&format!(
                        "Script {} tried to escape the scripts directory.",
                        request.script_path.display()
                    ));
                    continue;
                }

                let script_content = match fs::read_to_string(&filesystem_script_path).await {
                    Ok(v) => v,
                    Err(e) => {
                        report_error!(&format!(
                            "Failed to read script file for {}: {e}",
                            request.script_path.display()
                        ));
                        continue;
                    }
                };

                match lua.set_named_registry_value(
                    "epibloom.context.current_script_path",
                    request.script_path.clone(),
                ) {
                    Ok(v) => v,
                    Err(e) => {
                        report_error!(&format!(
                            "Failed to set epibloom.context.current_script_path registry value for {}: {e}",
                            request.script_path.display()
                        ));
                        continue;
                    }
                };

                let result = lua
                    .load(script_content)
                    .set_name(request.script_path.to_string_lossy())
                    .call_async(input)
                    .await;

                match result {
                    Ok(v) => {
                        update_status!(|status: &mut ScriptStatus| {
                            status.end_time = Some(OffsetDateTime::now_utc());

                            match lua.from_value_with(v, *DESERIALIZE_OPTIONS) {
                                Ok(v) => status.result = v,
                                Err(e) => {
                                    log::warn!(
                                        "Could not convert script result to JSON value: {e}"
                                    );
                                }
                            }
                        });
                    }
                    Err(e) => {
                        report_error!(&format!(
                            "Error executing script {} on context {}: {e}",
                            request.script_path.display(),
                            context_name,
                        ));
                    }
                }
            }
        }))?;

        Ok(Self {
            name: name.to_owned(),
            thread,
            script_request_sender,
            status_history,
        })
    }

    /// Returns the index into this context's script status history
    /// corresponding to this script execution.
    pub async fn run_script<P: AsRef<Path>>(
        &self,
        script_path: P,
        input: serde_json::Value,
    ) -> Result<usize, Error> {
        log::info!(
            "Queuing script {} on context {}",
            script_path.as_ref().display(),
            &self.name
        );

        let index = {
            let mut status_history = self.status_history.lock().await;

            let index = status_history.status_history.len();
            status_history.status_history.push(ScriptStatus {
                script_path: script_path.as_ref().to_string_lossy().into_owned(),

                request_time: OffsetDateTime::now_utc(),
                start_time: None,
                end_time: None,

                input: input.clone(),
                result: serde_json::Value::Null,

                error: None,
            });
            status_history.sync_to_state_object().await;

            index
        };

        self.script_request_sender
            .send(ScriptRunRequest::new(script_path, input, index))
            .await
            .map_err(|_| Error::ScriptQueueFull)?;

        Ok(index)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Script run queue for this context is full")]
    ScriptQueueFull,

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Lua error: {0}")]
    Lua(#[from] mlua::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
