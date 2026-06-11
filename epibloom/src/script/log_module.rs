use mlua::{Lua, ObjectLike, Table};

pub fn setup(lua: &Lua) -> mlua::Result<()> {
    let globals = lua.globals();
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

    Ok(())
}
