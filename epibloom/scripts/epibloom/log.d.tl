--- @meta

--- Logging to Epibloom's logging infrastructure.
--- @class loglib
local log = {}

--- Log a message to Epibloom's logging infrastructure.
--- @param level "error" | "warn" | "info" | "debug" | "trace"
--- @param message string
function log.log(level, message) end

--- Log at the `error` level.
--- @param message string # String that is passed through `string.format`.
--- @param ... any # Arguments for the format string.
function log.error(message, ...) end

--- Log at the `warn` level.
--- @param message string # String that is passed through `string.format`.
--- @param ... any # Arguments for the format string.
function log.warn(message, ...) end

--- Log at the `info` level.
--- @param message string # String that is passed through `string.format`.
--- @param ... any # Arguments for the format string.
function log.info(message, ...) end

--- Log at the `debug` level.
--- @param message string # String that is passed through `string.format`.
--- @param ... any # Arguments for the format string.
function log.debug(message, ...) end

--- Log at the `trace` level.
--- @param message string # String that is passed through `string.format`.
--- @param ... any # Arguments for the format string.
function log.trace(message, ...) end

return log
