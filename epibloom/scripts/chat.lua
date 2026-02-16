local fs = require("epibloom.filesystem")
local log = require("epibloom.log")
local state = require("epibloom.state")
local traces = require("epibloom.trace")
local uuid = require("epibloom.uuid")

local util = require("util")

local chat = {}

--- Create a new chat and return its initial state.
--- @return Chat
function chat.new()
    local new_chat_id = uuid.new()
    local c = chat.get(new_chat_id, true)
    chat.save(c)
    return c
end

--- @class Chat
--- @field id uuid
--- @field messages ChatMessage[]

--- @class ChatMessage
--- @field role Role
--- @field content string
--- @field trace_id TraceId|nil # The trace that was used to generate this message, if applicable.

--- Get the current state of a chat.
--- @param chat_id uuid
--- @param create_if_not_found boolean? # If false and the chat does not exist, an error is raised.
--- @return Chat
function chat.get(chat_id, create_if_not_found)
    local chat_state = chat.get_state(chat_id, create_if_not_found)
    return chat_state:read()
end

--- Get a list of all existing chats.
--- @return Chat[]
function chat.list_chats()
    local list = {}

    for _, file in ipairs(fs.files("chats/*.json")) do
        list[#list + 1] = file:match("chats/([0-9a-f%-]+).json")
    end

    return list
end

function chat.set_message(chat_id, message_position, role, content)
    local chat_state = chat.get_state(chat_id)

    chat_state:update(function(value)
        --- @cast value Chat

        if not message_position then
            message_position = #value.messages + 1
        end

        assert(message_position <= (#value.messages + 1), "tried to add message past the end of the message list")

        local old_message = value.messages[message_position]

        value.messages[message_position] = {
            role = role or old_message.role,
            content = content or old_message.content,
        }

        chat.save(value)
        return value
    end)
end

function chat.generate(chat_id, ...)
    local trace = traces.new()

    local chat_state = chat.get_state(chat_id)

    chat_state:update(function(value)
        --- @cast value Chat

        for i, message in ipairs(value.messages) do
            if message.role and message.content then
                trace[i] = { role = message.role, content = message.content }
            end
        end

        -- TODO: Mark tree branch as generating and check for this before generating.

        return value
    end)

    trace:generate(...)
    trace:wait_for_generate()

    local new_message = trace[#trace]
    if new_message then
        print(new_message.content)

        chat_state:update(function(value)
            --- @cast value Chat

            local last_message = value.messages[#value.messages]

            if last_message.role == "assistant" then
                -- Continuation of assistant message, append content.
                last_message.content = last_message.content .. new_message.content
                last_message.trace_id = trace.id
            else
                -- Normal assistant response after user message.
                value.messages[#value.messages + 1] = {
                    role = new_message.role,
                    content = new_message.content,
                    trace_id = trace.id,
                }
            end

            chat.save(value)
            return value
        end)
        return true
    else
        return false
    end
end

--- Get the state object associated with an existing chat, initializing it if
--- required.
--- @param chat_id uuid
--- @param create_if_not_found boolean? # If false and the chat does not exist, an error is raised.
--- @return StateObject # State object holding a Chat.
--- @private
function chat.get_state(chat_id, create_if_not_found)
    local chats = chat.list_chats()

    local state_name = "chat:" .. chat_id

    if util.table.find(chats, chat_id) then
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
        error("No chat exists with ID '" .. chat_id .. "'")
    end
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
