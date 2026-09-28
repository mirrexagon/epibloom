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

--- @alias ChatId string

--- The top-level structure of a chat JSON object that is stored in a state object.
--- @class Chat
--- @field name string
--- @field id ChatId
--- @field messages Message[]

--- @class Message
--- @field content MessageContent[]

--- @alias MessageContent
--- | string # Text.

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

--- Get the Chat object for a chat.
--- @param chat_id ChatId
--- @return Chat
function chat.get(chat_id)
    if not util.table.find(chat.list_all(), chat_id) then
        error(string.format("No existing chat with ID '%s' found.", chat_id))
    end

    local state_object = chat._get_state_object(chat_id)
    return state_object:read()
end

--- Modify/update the Chat object for a chat.
--- @param chat_id ChatId
--- @param update_function fun(chat: Chat)
function chat.update(chat_id, update_function)
    if not util.table.find(chat.list_all(), chat_id) then
        error(string.format("No existing chat with ID '%s' found.", chat_id))
    end

    local state_object = chat._get_state_object(chat_id)
    state_object:update(update_function)
end

function chat._get_state_object(chat_id)
    return state.get(string.format("chat:%s", chat_id))
end

-- Pass nil chat_id to get a new chat.

--- Blah
--- @param chat_id any
--- @return Chat
function Chat.get(chat_id)
    local chat = {}

    if chat_id then
        -- Confirm that a chat with this ID already exists.
        if not util.table.find(Chat.list(), chat_id) then
            error(string.format("No existing chat with ID '%s' found.", chat_id))
        end
    end

    chat.id = chat_id or uuid.now_v7()
    chat.state_object = Chat._get_state_object(chat.id)

    local now_string = time.now_utc_rfc3339()

    chat.state_object:update(function(value)
        if not value then
            -- This is a new chat.
            value = {
                id = chat.id,
                created = now_string,
                modified = now_string,
                messages = {},
            }
        end

        return value
    end)

    return setmetatable(chat, Chat)
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
