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
--- @field generating? boolean # `true` if this message is currently being generated and so its content cannot be modified at the moment. TODO: This conflates temporary state with persistent state, if the application dies in the middle of generating and leaves this `true`, on startup we'd want to reset it somehow. Perhaps use a separate state object? But we'd want to synchronize access to both the persistent and ephemeral one.

--- A list of message alternates/rerolls ("swipes").
--- @class MessageSlot
--- @field alternates Message[]

--- A single message.
--- @class Message: MessageInput
--- @field trace_id? uuid # Trace ID for the LLM generation that produced this message.
--- @field created string
--- @field modified string

--- @class MessageInput
--- @field role Role # Who this message is attributed to.
--- @field content MessageContent[]
--- @field note string # A user note attached to this message.

--- @alias MessageContent
--- | string[] # Text. This is a list for efficient chunk streaming via JSON Patch, the list elements should be concatenated to form the message text.

--- @alias Role
--- | "system"
--- | "user"
--- | "assistant"

--- List the IDs of all stored chats.
--- @return ChatId[]
function chat.list()
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
--- @param chat_id ChatId
--- @param message_index integer
--- @param new_message MessageInput
--- @param alternate_index integer
function chat.insert_message(chat_id, message_index, new_message, alternate_index)
    chat._update(chat_id, function(c)
        if message_index > #c.messages + 1 then
            util.errorf(
                "Tried to insert a message into chat %s at message index %d but there are only %d messages (provided index was more than that + 1).",
                chat_id,
                message_index,
                #c.messages
            )
        end

        if not c.messages[message_index] then
            c.messages[message_index] = {
                alternates = {},
            }
        end

        local message = c.messages[message_index]

        if alternate_index > #message.alternates + 1 then
            util.errorf(
                "Tried to insert a message into chat %s at message index %d alternate index %d but there are only %d alternates (provided index was more than that + 1).",
                chat_id,
                message_index,
                alternate_index,
                #message.alternates
            )
        end

        local new_message = new_message --[[@as Message]]
        local now_string = time.now_utc_rfc3339()

        new_message.created = now_string
        new_message.modified = now_string

        message.alternates[alternate_index] = new_message

        return c
    end)
end

--- Set the content of an existing message alternate.
--- @param chat_id ChatId
--- @param message_index integer
--- @param alternate_index integer
--- @param content? MessageContent[] # If specified, overwrites the current content with this, otherwise it is left unchanged.
--- @param note? string # If specified, overwrites the current note with this, otherwise it is left unchanged.
function chat.set_message_content(chat_id, message_index, alternate_index, content, note)
    chat._update(chat_id, function(c)
        if message_index > #c.messages then
            util.errorf(
                "Tried to set content for a message in chat %s at message index %d but there are only %d messages.",
                chat_id,
                message_index,
                #c.messages
            )
        end

        if not c.messages[message_index] then
            c.messages[message_index] = {
                alternates = {},
            }
        end

        local message = c.messages[message_index]

        if alternate_index > #message.alternates then
            util.errorf(
                "Tried to set content for a message in chat %s at message index %d alternate index %d but there are only %d alternates.",
                chat_id,
                alternate_index,
                #message.alternates
            )
        end

        local alternate = message.alternates[alternate_index]

        local now_string = time.now_utc_rfc3339()

        alternate.modified = now_string
        if content then
            alternate.content = content
        end
        if note then
            alternate.note = note
        end

        return c
    end)
end

function chat.generate(chat_id)
    chat._update(chat_id, function(c)
        return c
    end)
end

function chat.cancel_generate(chat_id) end

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
--- @param update_function fun(chat: Chat): Chat
--- @private
function chat._update(chat_id, update_function)
    if not util.table.find(chat.list_all(), chat_id) then
        error(string.format("No existing chat with ID '%s' found.", chat_id))
    end

    local state_object = chat._get_state_object(chat_id)
    state_object:update(update_function)
end

-- TODO: Have both chat:<id>:persist and chat:<id> objects. The :persist object
-- stores most things and the non-persist one is for things like "currently
-- generating" that should be reset if the application is closed eg. mid
-- generation.

--- Get the state object that is or would be associated with the given chat ID.
--- @param chat_id ChatId
--- @return StateObject
--- @private
function chat._get_state_object(chat_id)
    return state.get(string.format("chat:%s", chat_id))
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
