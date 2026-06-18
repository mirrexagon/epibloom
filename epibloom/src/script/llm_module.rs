use std::sync::Arc;

use futures::FutureExt;
use mlua::{FromLua, IntoLuaMulti, Lua, LuaSerdeExt, ObjectLike, Table, UserData, Value};
use tokio::sync::mpsc;

use crate::backend::anthropic;
use crate::llm;

pub fn setup(lua: &Lua, llm_store: Arc<llm::LlmStore>) -> mlua::Result<()> {
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
        lua.create_async_function(async |lua, ()| {
            let store: LuaLlmStore = lua.named_registry_value("epibloom.llm.store")?;
            let anthropic_client = Arc::clone(&store.llm_store.anthropic_client);

            match anthropic_client.list_models().await {
                Ok(response) => Ok((
                    // TODO: Replace uses of to_value_with with implementing IntoLua for references to the thing.
                    Some(lua.to_value_with(&response, *super::SERIALIZE_OPTIONS)?),
                    None,
                )),
                Err(err) => Ok((None, Some(err.to_string()))),
            }
        })?,
    )?;

    // TODO: Allow this to run in the background, maybe have the generate handle
    // either hold a future for a response or the receive channel/stream for
    // streaming.
    // Store task and cancellation token.
    llm_module.set(
        "anthropic_generate",
        lua.create_async_function(async |lua, request: Value| {
            let request: anthropic::messages::request::MessagesRequest =
                lua.from_value_with(request, *super::DESERIALIZE_OPTIONS)?;

            let store: LuaLlmStore = lua.named_registry_value("epibloom.llm.store")?;

            let handle = store
                .llm_store
                .anthropic_generate(request)
                .await
                .map_err(mlua::Error::external)?;

            Ok(LuaAnthropicGenerateHandle { handle })
        })?,
    )?;

    llm_module.set(
        "anthropic_generate_streaming",
        lua.create_async_function(async |lua, request: Value| {
            let request: anthropic::messages::request::MessagesRequest =
                lua.from_value_with(request, *super::DESERIALIZE_OPTIONS)?;

            let store: LuaLlmStore = lua.named_registry_value("epibloom.llm.store")?;
            let handle = store
                .llm_store
                .anthropic_generate_streaming(request)
                .await
                .map_err(mlua::Error::external)?;

            Ok(LuaAnthropicGenerateStreamingHandle {
                handle,
                response: None,
            })
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
    llm_store: Arc<llm::LlmStore>,
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

/// Handle returned by `llm.anthropic_generate()`.
/// TODO: Just implement UserData for the underlying handle directly.
struct LuaAnthropicGenerateHandle {
    handle: llm::AnthropicGenerateHandle,
}

impl UserData for LuaAnthropicGenerateHandle {
    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        methods.add_async_method_mut("get_response", async |lua, mut lua_handle, ()| {
            match lua_handle.handle.get_response().await {
                Ok(response) => Ok((
                    Some(lua.to_value_with(response, *super::SERIALIZE_OPTIONS)?),
                    None,
                )),
                Err(err) => Ok((None, Some(err.to_string()))),
            }
        });
    }
}

/// Handle returned by `llm.anthropic_generate_streaming()`.
/// TODO: Just implement UserData for the underlying handle directly.
struct LuaAnthropicGenerateStreamingHandle {
    handle: llm::AnthropicGenerateStreamingHandle,

    /// The final full response.
    // TODO: Build the final response up from the streaming events and expose to it Lua as a property on this.
    response: Option<anthropic::messages::Message>,
}

impl UserData for LuaAnthropicGenerateStreamingHandle {
    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        // __call: return the next event or whatever error occurs, then nil when
        // generation is complete or after an error.
        methods.add_async_meta_method_mut("__call", async |lua, mut lua_handle, ()| {
            let result = match lua_handle.handle.get_next_event().await {
                Some(event) => match event {
                    Ok(event) => StreamingItem::Event(&event),
                    Err(err) => StreamingItem::Error(err),
                },
                None => StreamingItem::Done,
            };

            Ok(result.into_lua_multi(&lua))
        });

        // wait(): drain remaining events, returning (response or nil, nil or error message).
        methods.add_async_method_mut("get_response", async |lua, mut lua_handle, ()| {
            let (response, error) = lua_handle.handle.get_response().await;

            Ok((
                response
                    .map(|r| lua.to_value_with(r, *super::SERIALIZE_OPTIONS))
                    .transpose()?,
                error.map(|e| e.to_string()),
            ))
        });
    }
}

#[derive(Debug)]
enum StreamingItem<'a> {
    Event(&'a anthropic::messages::response::MessageStreamEvent),
    Error(&'a anthropic::Error),
    Done,
}

impl<'a> IntoLuaMulti for StreamingItem<'a> {
    fn into_lua_multi(self, lua: &Lua) -> mlua::Result<mlua::MultiValue> {
        // Event => true, event
        // Error => false, error
        // Done => nil
        todo!()
    }
}
