--- @meta

--- Functions related to date and time.
--- @class EpibloomTimeModule
local time = {}

--- Get the current UTC date and time as an RFC 3339 string, eg. `2025-01-01T12:34:56.789Z`.
--- @return string
--- @nodiscard
function time.now_utc_rfc3339() end

--- Asynchronously sleep for the given number of seconds.
---
--- Unlike eg. `os.execute("sleep 1")`, this doesn't block the script context's thread. It is okay to block the thread, but better to avoid it if you don't need to.
--- @param seconds number # Number of seconds to sleep for. Can be fractional. An error is raised if this is negative or not finite.
function time.sleep(seconds) end

return time
