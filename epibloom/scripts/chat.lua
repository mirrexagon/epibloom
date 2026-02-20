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
--- @field thinking string[]? # Thinking text as a list of string chunks, if present. Join them to get the full text.
--- @field content string[] # Message content as a list of string chunks. Join them to get the full text.
--- @field trace_id TraceId? # The trace that was used to generate this message, if applicable.
--- @field error string? # If an error occurred generating the message, this will have the error message.

--- Get the current state of a chat.
--- @param chat_id uuid
--- @param create_if_not_found boolean? # If false and the chat does not exist, an error is raised.
--- @return Chat
function chat.get(chat_id, create_if_not_found)
    local chat_state = chat.get_state(chat_id, create_if_not_found)
    return chat_state:read()
end

function chat.poke(chat_id)
    chat.get_state(chat_id)
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

        -- Content is stored as a list of string chunks for efficient streaming.
        -- When provided as a plain string (e.g. from user input), wrap it in a
        -- single-element list.
        local new_content
        if content then
            if type(content) == "string" then
                new_content = { content }
            else
                new_content = content
            end
        else
            new_content = old_message and old_message.content or {}
        end

        value.messages[message_position] = {
            role = role or (old_message and old_message.role),
            content = new_content,
        }

        chat.save(value)
        return value
    end)
end

function chat.generate(chat_id, ...)
    local chat_state = chat.get_state(chat_id)

    local trace, generate_handle, message_position

    -- Copy the current chat messages into the trace. Content is joined from
    -- its chunk array into a plain string, as required by the trace/LLM layer.
    chat_state:update(function(value)
        --- @cast value Chat

        -- TODO: Mark tree branch as generating, and check that flag before generating.

        local generating_message = value.messages[#value.messages]

        if generating_message and generating_message.role == "assistant" then
            -- Continuation of assistant message, reuse trace if possible.
            trace = traces.get(generating_message.trace_id)
            if not trace then
                trace = traces.new()
            end
            generating_message.trace_id = trace.id
            generating_message.error = nil
        else
            -- Normal assistant response after a user message.
            trace = traces.new()
            generating_message = {
                role = "assistant",
                content = {},
                trace_id = trace.id,
            }

            value.messages[#value.messages + 1] = generating_message
        end

        for i, message in ipairs(value.messages) do
            if message == generating_message then
                break
            end

            if message.role and message.content then
                trace[i] = { role = message.role, content = table.concat(message.content) }
            end
        end

        generate_handle = trace:generate("anthropic")
        message_position = #value.messages

        return value
    end)

    -- Stream chunks from the LLM into the chat state. Each chunk triggers a
    -- small JSON Patch (array-element add) rather than replacing the whole
    -- content string, keeping per-chunk SSE payloads small.
    for chunk in generate_handle do
        --- @cast chunk TraceGenerateChunk

        chat_state:update(function(value)
            --- @cast value Chat

            local message = value.messages[message_position]

            if chunk.type == "message" then
                message.content[#message.content + 1] = chunk.delta
            elseif chunk.type == "thinking" then
                if not message.thinking then
                    message.thinking = {}
                end

                message.thinking[#message.thinking + 1] = chunk.delta
            elseif chunk.type == "error" then
                message.error = chunk.message
                log.error("Error in generation: %s", chunk.message)
            end

            return value
        end)
    end

    -- Persist to disk once, after the full response has been received.
    local final_value = chat_state:read()
    if final_value and final_value.messages and #final_value.messages > 0 then
        chat.save(final_value)
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
            return value or chat.load(chat_id)
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
