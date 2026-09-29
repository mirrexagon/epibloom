package.loaded["util"] = nil
package.loaded["chat"] = nil

local chat = require("chat")

local arg = ...

assert(type(arg) == "table", "expected object, got " .. type(arg))
assert(type(arg.action) == "string", "Field 'action' missing or not a string.")

if arg.action == "new_chat" then
    chat.new(arg.name)
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
