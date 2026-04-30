--- Manage state objects that send updates to all clients when modified.
local record state
    --- A wrapper for a JSON value that sends updates to connected clients when modified.
    record StateObject
        --- Get a copy of the current value.
        read: function(StateObject): any

        --- Update the held value, updating any connected clients when the update is complete.
        --- `update_function` is a function that takes a copy of the current
        --- held value and must return the updated value.
        update: function(StateObject, update_function: function(value: any): any)
    end

    --- Get the named state object. It will be created if it does not already exist.
    --- By convention, state object names are multi-part, separated by colons - eg. `chat:123:messages`.
    get: function(name: string): StateObject
end

return state
