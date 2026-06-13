use std::sync::Arc;

use mlua::{FromLua, Lua, LuaSerdeExt, ObjectLike, Table, UserData, Value};
use tokio::sync::mpsc;

use crate::backend::anthropic;
use crate::llm::{self, LlmStore};

pub fn setup(lua: &Lua, llm_store: Arc<LlmStore>) -> mlua::Result<()> {
    lua.set_named_registry_value(
        "epibloom.llm.store",
        lua.create_userdata(LuaLlmStore {
            llm_store: Arc::clone(&llm_store),
        })?,
    )?;

    let globals = lua.globals();
    let llm_module = lua.create_table()?;

    llm_module.set(
        "anthropic_list_models",
        lua.create_async_function(async |lua, _: ()| {
            let store: LuaLlmStore = lua.named_registry_value("epibloom.llm.store")?;
            let anthropic_client = Arc::clone(&store.llm_store.anthropic_client);

            match anthropic_client.list_models().await {
                Ok(response) => Ok((
                    Some(lua.to_value_with(&response, *super::SERIALIZE_OPTIONS)?),
                    None,
                )),
                Err(err) => Ok((None, Some(err.to_string()))),
            }
        })?,
    )?;

    llm_module.set(
        "anthropic_generate",
        lua.create_async_function(async |lua, request: Value| {
            let request: anthropic::messages::request::MessagesRequest =
                lua.from_value_with(request, *super::DESERIALIZE_OPTIONS)?;

            let store: LuaLlmStore = lua.named_registry_value("epibloom.llm.store")?;
            match store.llm_store.anthropic_generate(request).await {
                Ok(response_message) => Ok((
                    Some(lua.to_value_with(&response_message, *super::SERIALIZE_OPTIONS)?),
                    None,
                )),
                Err(err) => Ok((None, Some(err.to_string()))),
            }
        })?,
    )?;

    llm_module.set(
        "anthropic_generate_streaming",
        lua.create_async_function(async |lua, request: Value| {
            let request: anthropic::messages::request::MessagesRequest =
                lua.from_value_with(request, *super::DESERIALIZE_OPTIONS)?;

            let store: LuaLlmStore = lua.named_registry_value("epibloom.llm.store")?;
            let rx = store
                .llm_store
                .anthropic_generate_streaming(request)
                .await
                .map_err(mlua::Error::external)?;

            Ok(LuaGenerateAnthropicHandle { receiver: Some(rx) })
        })?,
    )?;

    llm_module.set(
        "anthropic_count_tokens",
        lua.create_async_function(async |lua, request: Value| {
            let request: anthropic::messages::request::CountTokensRequest =
                lua.from_value_with(request, *super::DESERIALIZE_OPTIONS)?;

            let store: LuaLlmStore = lua.named_registry_value("epibloom.llm.store")?;
            let anthropic_client = Arc::clone(&store.llm_store.anthropic_client);

            match anthropic_client.count_tokens(request).await {
                Ok(response) => Ok((
                    Some(lua.to_value_with(&response, *super::SERIALIZE_OPTIONS)?),
                    None,
                )),
                Err(err) => Ok((None, Some(err.to_string()))),
            }
        })?,
    )?;

    let package_loaded: Table = globals.get_path("package.loaded")?;
    package_loaded.set("epibloom.llm", llm_module)?;

    Ok(())
}

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

/// Handle returned by `llm.generate_anthropic()`. Supports chunk-by-chunk iteration
/// (via `__call`) and blocking until complete (via `wait()`).
struct LuaGenerateAnthropicHandle {
    receiver: Option<mpsc::Receiver<llm::AnthropicMessagesEvent>>,
    // TODO: Save the response/build it up from the streaming events and expose to it Lua as a property on this.
}

impl UserData for LuaGenerateAnthropicHandle {
    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        // __call: return the next chunk, or nil when generation is complete.
        methods.add_async_meta_method_mut(
            "__call",
            async |lua, mut handle, _args: mlua::MultiValue| {
                // Receive one chunk. The borrow of `handle.receiver` via
                // `ref mut rx` ends before we call `.take()` below.
                let event = match handle.receiver {
                    None => return Ok(None),
                    Some(ref mut rx) => rx.recv().await,
                };

                match event {
                    Some(event) => match event {
                        llm::AnthropicMessagesEvent::MessageStreamEvent(event) => {
                            Ok(Some(lua.to_value_with(&event, *super::SERIALIZE_OPTIONS)?))
                        }
                        llm::AnthropicMessagesEvent::Error(message) => {
                            let result = lua.create_table()?;
                            result.set("type", "error")?;
                            result.set("message", message)?;
                            Ok(Some(Value::Table(result)))
                        }
                    },
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
                Some(llm::AnthropicMessagesEvent::Error(message)) => {
                    handle.receiver.take();
                    return Ok((false, Some(message)));
                }
                Some(_) => {} // Discard normal chunks.
            }
        });
    }
}
