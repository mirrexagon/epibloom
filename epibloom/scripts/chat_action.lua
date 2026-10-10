package.loaded["util"] = nil
package.loaded["chat"] = nil

local chat = require("chat")

local arg = ...

assert(type(arg) == "table", "expected object, got " .. type(arg))
assert(type(arg.action) == "string", "Field 'action' missing or not a string.")

--- Build a `MessageInput` from the request. Accepts either `content` (a list of
--- lists of strings) or `text` (a plain string) as a shorthand.
--- @return MessageInput
local function message_input()
    local content = arg.content
    if content == nil and arg.text ~= nil then
        assert(type(arg.text) == "string", "Field 'text' must be a string.")
        content = { { arg.text } }
    end

    return { role = arg.role, content = content, note = arg.note }
end

--- Like `message_input()`, but returns `nil` content if neither `content` nor
--- `text` were given (meaning "leave unchanged").
local function optional_content()
    if arg.content ~= nil then
        return arg.content
    elseif arg.text ~= nil then
        assert(type(arg.text) == "string", "Field 'text' must be a string.")
        return { { arg.text } }
    end
    return nil
end

local actions = {
    list = function()
        return chat.list()
    end,

    get = function()
        return chat.get(arg.chat_id)
    end,

    new = function()
        return { chat_id = chat.new(arg.name) }
    end,

    set_name = function()
        chat.set_name(arg.chat_id, arg.name)
    end,

    set_system = function()
        chat.set_system(arg.chat_id, arg.system)
    end,

    set_settings = function()
        chat.set_settings(arg.chat_id, { model = arg.model, max_tokens = arg.max_tokens })
    end,

    append_message = function()
        return { message_index = chat.append_message(arg.chat_id, message_input()) }
    end,

    insert_message = function()
        return { message_index = chat.insert_message(arg.chat_id, arg.message_index, message_input()) }
    end,

    insert_alternate = function()
        return {
            alternate_index = chat.insert_alternate(
                arg.chat_id,
                arg.message_index,
                message_input(),
                arg.alternate_index
            ),
        }
    end,

    set_message_content = function()
        chat.set_message_content(arg.chat_id, arg.message_index, arg.alternate_index, optional_content(), arg.note)
    end,

    select_alternate = function()
        chat.select_alternate(arg.chat_id, arg.message_index, arg.alternate_index)
    end,

    delete_alternate = function()
        chat.delete_alternate(arg.chat_id, arg.message_index, arg.alternate_index)
    end,

    generate = function()
        return chat.generate(arg.chat_id)
    end,

    cancel_generate = function()
        return { was_generating = chat.cancel_generate(arg.chat_id) }
    end,
}

local action = actions[arg.action]
if not action then
    error(string.format("Unknown action: %s", arg.action))
end

return action()
