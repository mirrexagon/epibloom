--- @meta

--- Manage state objects that send updates to all clients when modified.
--- @class EpibloomStateModule
local state = {}

--- Get the named state object. It will be created if it does not already exist.
---
--- State objects whose names end in `:persist` are automatically saved to disk and loaded on startup.
---
--- The state object with the empty name `""` is the index. Its value is a sorted array of the names of all known state objects.
--- @param name string # The name of the state object to get. By convention, names are multi-part, separated by colons - eg. `chat:123:messages`.
--- @return StateObject
--- @nodiscard
function state.get(name) end

--- A wrapper for a JSON value that sends updates to connected clients when modified.
--- @class StateObject
--- @field name string # The name of the state object, as passed to `state.get()`.
local StateObject = {}

--- Get a copy of the current value.
---
--- Note that JSON `null`s are not represented, so an object key with a null value will be missing entirely.
--- @return any
--- @nodiscard
function StateObject:read() end

--- Atomically update the held value, updating any connected clients when the update is complete.
---
--- Empty tables are converted to JSON as empty arrays.
--- @param update_function async fun(value: any): any # A function that takes a copy of the current held value and must return the updated value.
function StateObject:update(update_function) end

return state
