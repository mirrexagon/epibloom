local llm = require("epibloom.llm")
local log = require("epibloom.log")
local state = require("epibloom.state")
local time = require("epibloom.time")
local uuid = require("epibloom.uuid")

local util = require("util")

local chat = {}

--[[
TODO: DESIGN

Each message index has one or more "swipes" and a selected swipe index.

--]]

--- @alias ChatId uuid

--- The top-level structure of a chat JSON object that is stored in a state object.
--- @class Chat
--- @field id ChatId
--- @field name string
--- @field messages MessageSlot[]
--- @field created string
--- @field modified string

--- A list of message alternates/rerolls ("swipes").
--- @class MessageSlot
--- @field alternates Message[]

--- A single message.
--- @class Message
--- @field role Role # Who this message is attributed to.
--- @field content MessageContent[]
--- @field trace_id? uuid # Trace ID for the LLM generation that produced this message.
--- @field generating boolean # `true` if this message is currently being generated.
--- @field created string
--- @field modified string

--- @alias MessageContent
--- | string # Text.

--- @alias Role
--- | "system"
--- | "user"
--- | "assistant"

--- List the IDs of all stored chats.
--- @return ChatId[]
function chat.list_all()
    return util.table.filter_map(state.get(""):read(), function(name)
        if string.find(name, "^chat:") then
            return true, string.match(name, "^chat:(.*)$")
        else
            return false
        end
    end)
end

--- Create a new chat.
--- @param name? string
function chat.new(name)
    local chat_id = uuid.now_v7()

    local now_string = time.now_utc_rfc3339()

    --- @type Chat
    local new_chat = {
        id = uuid.now_v7(),
        name = name or now_string,

        messages = {},

        created = now_string,
        modified = now_string,
    }

    local state_object = chat._get_state_object(chat_id)
    state_object:update(function()
        return new_chat
    end)
end

--- Insert or append a new message or message alternate.
---
--- Set an index one past the end to append.
--- @param chat_id any
--- @param message_index any
--- @param alternate_index any
function chat.insert_message(chat_id, message_index, alternate_index) end

--- Set the content of an existing message alternate.
--- @param chat_id any
--- @param message_index any
--- @param alternate_index any
function chat.set_message(chat_id, message_index, alternate_index) end

--- Get the Chat object for a chat.
--- @param chat_id ChatId
--- @return Chat
--- @private
function chat._get(chat_id)
    if not util.table.find(chat.list_all(), chat_id) then
        error(string.format("No existing chat with ID '%s' found.", chat_id))
    end

    local state_object = chat._get_state_object(chat_id)
    return state_object:read()
end

--- Modify/update the Chat object for a chat.
--- @param chat_id ChatId
--- @param update_function fun(chat: Chat)
--- @private
function chat._update(chat_id, update_function)
    if not util.table.find(chat.list_all(), chat_id) then
        error(string.format("No existing chat with ID '%s' found.", chat_id))
    end

    local state_object = chat._get_state_object(chat_id)
    state_object:update(update_function)
end

--- Get the state object that is or would be associated with the given chat ID.
--- @param chat_id ChatId
--- @return StateObject
--- @private
function chat._get_state_object(chat_id)
    return state.get(string.format("chat:%s", chat_id))
end

function Chat:append_message(role, text)
    assert(role == "user" or role == "assistant", string.format('role: expected "user" or "assistant", got %s', role))
    assert(type(text) == "string", string.format("text: expected string, got %s", type(text)))

    self.state_object:update(function(value)
        table.insert(value.messages, { role = role, text = text })
        return value
    end)
end

function Chat:generate()
    local request = {
        model = "claude-haiku-4-5",
        max_tokens = 128,
        system = "You are a helpful assistant catgirl! Nyaa!~ Be concise, and always ask a question at the end of your responses!",

        messages = util.table.filter_map(self.state_object:read().messages, function(m)
            return true, {
                role = m.role,
                content = m.text,
            }
        end),
    }

    local response = assert(llm.anthropic_generate(request))

    self.state_object:update(function(value)
        table.insert(value.messages, { role = response.role, text = response.content[1].text })
        value.modified = time.now_utc_rfc3339()
        return value
    end)
end

return chat
