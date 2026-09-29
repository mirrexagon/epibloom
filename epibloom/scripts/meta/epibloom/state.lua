--- @meta

--- Manage state objects that send updates to all clients when modified.
--- @class statelib
local state = {}

--- Get the named state object. It will be created if it does not already exist.
--- @param name string # The name of the state object to get. By convention, names are multi-part, separated by colons - eg. `chat:123:messages`.
--- @return StateObject
--- @nodiscard
function state.get(name) end

--- A wrapper for a JSON value that sends updates to connected clients when modified.
--- @class StateObject
--- @field name string # The name of the state object - the same one passed to `state.get()`.
local StateObject = {}

--- Get a copy of the current value.
--- @return any
--- @nodiscard
function StateObject:read() end

--- Update the held value, updating any connected clients when the update is complete.
--- @param update_function fun(value: any): any # A function that takes a copy of the current held value and must return the updated value.
function StateObject:update(update_function) end

return state
