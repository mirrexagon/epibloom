--- @meta

--- Logging to Epibloom's logging infrastructure.
---
--- Log messages are tagged with the script path and script context name.
---
--- The global `print()` is also replaced to log its arguments (converted with `tostring()` and separated by tabs) at the `info` level.
--- @class EpibloomLogModule
local log = {}

--- @alias LogLevel "error" | "warn" | "info" | "debug" | "trace"

--- Log a message to Epibloom's logging infrastructure.
--- @param level LogLevel
--- @param message string
function log.log(level, message) end

--- Log at the `error` level.
--- @param message any # Converted with `tostring` and passed through `string.format`.
--- @param ... any # Arguments for the format string.
function log.error(message, ...) end

--- Log at the `warn` level.
--- @param message any # Converted with `tostring` and passed through `string.format`.
--- @param ... any # Arguments for the format string.
function log.warn(message, ...) end

--- Log at the `info` level.
--- @param message any # Converted with `tostring` and passed through `string.format`.
--- @param ... any # Arguments for the format string.
function log.info(message, ...) end

--- Log at the `debug` level.
--- @param message any # Converted with `tostring` and passed through `string.format`.
--- @param ... any # Arguments for the format string.
function log.debug(message, ...) end

--- Log at the `trace` level.
--- @param message any # Converted with `tostring` and passed through `string.format`.
--- @param ... any # Arguments for the format string.
function log.trace(message, ...) end

return log
