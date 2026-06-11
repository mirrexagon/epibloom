use std::sync::Arc;

use mlua::{FromLua, Lua, LuaSerdeExt, ObjectLike, Table, UserData, Value};

use crate::state::{StateObject, StateStore};

pub fn setup(lua: &Lua, state_store: Arc<StateStore>) -> mlua::Result<()> {
    let globals = lua.globals();

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

                let state_obj = state_store
                    .state_store
                    .get_or_create(&name)
                    .await
                    .map_err(mlua::Error::external)?;

                Ok(LuaStateObject {
                    name: name.clone(),
                    state_object: state_obj,
                })
            })?,
        )?;

        let package_loaded: Table = globals.get_path("package.loaded")?;
        package_loaded.set("epibloom.state", state_store_proxy_module)?;
    }

    Ok(())
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
        methods.add_async_method("get", |lua, obj, ()| async move {
            let value = obj.state_object.get().await;
            let lua_value = lua.to_value_with(&value, *super::SERIALIZE_OPTIONS)?;
            Ok(lua_value)
        });

        // Atomic read-modify-write helper
        methods.add_async_method("update", |lua, obj, func: mlua::Function| async move {
            obj.state_object
                .update(async |old_value| {
                    let lua_old_value = lua.to_value_with(&old_value, *super::SERIALIZE_OPTIONS)?;
                    let lua_new_value: Value = func.call_async(lua_old_value).await?;
                    let value = lua.from_value_with(lua_new_value, *super::DESERIALIZE_OPTIONS)?;
                    Ok(value)
                })
                .await
        });
    }
}
