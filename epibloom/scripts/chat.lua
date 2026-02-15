local fs = require("epibloom.filesystem")
local log = require("epibloom.log")
local state = require("epibloom.state")
local traces = require("epibloom.trace")
local uuid = require("epibloom.uuid")

local util = require("util")

local chat = {}

--- Create a new chat and return its state object.
--- @return State|nil
--- @return nil|string # nil on success, error message on failure.
function chat.new()
    local new_chat_id = uuid.new()
    assert(chat.get_state(new_chat_id, true))
    return new_chat_id
end

function chat.get(chat_id)
    local chat_state = assert(chat.get_state(chat_id))
    return chat_state:read()
end

function chat.list_chats()
    local list = {}

    for _, file in ipairs(fs.files("chats/*.json")) do
        list[#list + 1] = file:match("chats/([0-9a-f%-]+).json")
    end

    return list
end

--- Get the state object associated with an existing chat, initializing it if
--- required.
--- @param chat_id uuid
--- @param create_if_not_found boolean?
--- @return State|nil # Chat state object on success, nil on failure.
--- @return nil|string # nil on success, error message on failure.
--- @private
function chat.get_state(chat_id, create_if_not_found)
    local chats = chat.list_chats()

    local state_name = "chat:" .. chat_id

    if util.table.has(chats, chat_id) then
        local chat_state = state.get(state_name)

        chat_state:update(function(value)
            if not value then
                -- Read chat from file and put it in the state.
                return chat.load(chat_id)
            else
                return value
            end
        end)

        return chat_state
    elseif create_if_not_found then
        local chat_state = state.get(state_name)

        chat_state:update(function(_)
            return { id = chat_id, messages = {} }
        end)

        return chat_state
    else
        return nil, "No chat exists with ID '" .. chat_id .. "'"
    end
end

function chat.set_message(chat_id, message_position, message)
    local chat_state = assert(chat.get_state(chat_id))

    chat_state:update(function(value)
        if not message_position then
            message_position = #value.messages + 1
        end

        assert(message_position <= (#value.messages + 1), "tried to add message past the end of the message list")

        value.messages[message_position] = message

        chat.save(value)
        return value
    end)
end

function chat.generate(chat_id, ...)
    local trace = traces.new()

    local chat_state = assert(chat.get_state(chat_id))

    chat_state:update(function(value)
        for i, message in ipairs(value.messages) do
            print(i, message.role, message.content)

            trace[i] = message
        end

        value.messages[#value.messages + 1] = { trace_id = trace.id, role = "assistant", content = "" }

        return value
    end)

    trace:generate(...)
    trace:wait_for_generate()

    local new_message = trace[#trace]
    log.info(new_message.content)

    chat_state:update(function(value)
        value.messages[#value.messages].role = new_message.role
        value.messages[#value.messages].content = new_message.content

        chat.save(value)
        return value
    end)
end

--- @private
function chat.load(chat_id)
    return fs.load_json(string.format("chats/%s.json", chat_id))
end

--- @private
function chat.save(chat_state_contents)
    assert(chat_state_contents.id, "Tried to save chat content that has no id")
    fs.save_json(string.format("chats/%s.json", chat_state_contents.id), chat_state_contents)
end

return chat
