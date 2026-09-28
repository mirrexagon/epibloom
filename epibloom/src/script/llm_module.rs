use std::sync::Arc;

use mlua::{FromLua, Lua, LuaSerdeExt, ObjectLike, Table, UserData, Value};

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

            Ok(LuaAnthropicGenerateStreamingHandle { handle })
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
    fn add_fields<F: mlua::UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("name", |_lua, lua_handle| {
            Ok(lua_handle.handle.get_trace_state_object_name().to_owned())
        });
    }

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

        methods.add_method("cancel", |_lua, lua_handle, ()| {
            lua_handle.handle.cancel();
            Ok(())
        });
    }
}

/// Handle returned by `llm.anthropic_generate_streaming()`.
/// TODO: Just implement UserData for the underlying handle directly.
struct LuaAnthropicGenerateStreamingHandle {
    handle: llm::AnthropicGenerateStreamingHandle,
}

impl UserData for LuaAnthropicGenerateStreamingHandle {
    fn add_fields<F: mlua::UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("name", |_lua, lua_handle| {
            Ok(lua_handle.handle.get_trace_state_object_name().to_owned())
        });
    }

    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        // __call: return the next event or whatever error occurs, then nil when
        // generation is complete or after an error.
        methods.add_async_meta_method_mut("__call", async |lua, mut lua_handle, ()| {
            match lua_handle.handle.get_next_event().await {
                Some(event) => match event {
                    Ok(event) => {
                        // Event => true, event
                        let mut values = mlua::MultiValue::new();
                        values.push_back(Value::Boolean(true));
                        values.push_back(lua.to_value_with(&event, *super::SERIALIZE_OPTIONS)?);
                        Ok(values)
                    }
                    Err(err) => {
                        // Error => false, error string
                        let mut values = mlua::MultiValue::new();
                        values.push_back(Value::Boolean(false));
                        values.push_back(Value::String(lua.create_string(err.to_string())?));
                        Ok(values)
                    }
                },
                None => {
                    // Done => nil
                    let mut values = mlua::MultiValue::new();
                    values.push_back(Value::Nil);
                    Ok(values)
                }
            }
        });

        // get_response(): drain remaining events, returning (response or nil, nil or error message).
        methods.add_async_method_mut("get_response", async |lua, mut lua_handle, ()| {
            let (response, error) = lua_handle.handle.get_response().await;

            Ok((
                response
                    .map(|r| lua.to_value_with(r, *super::SERIALIZE_OPTIONS))
                    .transpose()?,
                error.map(|e| e.to_string()),
            ))
        });

        methods.add_method("cancel", |_lua, lua_handle, ()| {
            lua_handle.handle.cancel();
            Ok(())
        });
    }
}
