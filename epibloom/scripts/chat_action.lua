package.loaded["util"] = nil
package.loaded["Chat"] = nil

local Chat = require("Chat")

local arg = ...

assert(type(arg) == "table", "expected object, got " .. type(arg))
assert(type(arg.action) == "string", "Field 'action' missing or not a string.")

if arg.action == "list" then
    return Chat.list()
elseif arg.action == "new_chat" then
    local chat = Chat.get()
    return { chat_id = chat.id }
elseif arg.action == "append_message" then
    assert(type(arg.chat_id) == "string", "Field 'chat_id' not specified correctly.")

    local chat = Chat.get(arg.chat_id)
    return chat:append_message(arg.role, arg.text)
elseif arg.action == "generate" then
    assert(type(arg.chat_id) == "string", "Field 'chat_id' not specified correctly.")

    local chat = Chat.get(arg.chat_id)
    return chat:generate()
else
    error(string.format("Unknown action: %s", arg.action))
end
