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
--- @field messages table<uuid, ChatMessage>

--- @class ChatMessage
--- @field id uuid # Unique node identifier.
--- @field parent_id uuid? # Parent message ID; nil only for root nodes.
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
--- @return uuid[]
function chat.list_chats()
    local list = {}

    for _, file in ipairs(fs.files("chats/*.json")) do
        list[#list + 1] = file:match("chats/([0-9a-f%-]+).json")
    end

    return list
end

--- Add a new message node to the chat tree and return its ID.
--- @param chat_id uuid
--- @param parent_id uuid? # The parent message ID, or nil for a root node.
--- @param role Role
--- @param content string | string[] | nil
--- @return uuid # The new message ID.
function chat.add_message(chat_id, parent_id, role, content)
    local chat_state = chat.get_state(chat_id)

    -- NOTE: This module assumes that this UUID is UUIDv7, which is
    -- time-orderable, to record the order multiple branch messages coming off a
    -- single parent message.
    local new_id = uuid.new()

    chat_state:update(function(value)
        --- @cast value Chat

        local new_content
        if content then
            if type(content) == "string" then
                new_content = { content }
            else
                new_content = content
            end
        else
            new_content = {}
        end

        value.messages[new_id] = {
            id = new_id,
            parent_id = parent_id,
            role = role,
            content = new_content,
        }

        chat.save(value)
        return value
    end)

    return new_id
end

--- Walk parent_id links from node_id back to the root and return a
--- root-first ordered array of nodes.
--- @param messages table<uuid, ChatMessage>
--- @param node_id uuid
--- @return ChatMessage[]
function chat.get_path(messages, node_id)
    local path = {}
    local current_id = node_id

    while current_id do
        local node = messages[current_id]
        if not node then
            break
        end
        table.insert(path, 1, node)
        current_id = node.parent_id
    end

    return path
end

--- Generate an assistant response as a child of parent_message_id.
--- @param chat_id uuid
--- @param parent_message_id uuid # The user-message node to respond to.
function chat.generate(chat_id, parent_message_id)
    local chat_state = chat.get_state(chat_id)

    local trace, generate_handle, new_message_id

    -- Create the empty assistant node and start generation inside one atomic
    -- update so the SSE push reaches clients before streaming starts.
    chat_state:update(function(value)
        --- @cast value Chat

        new_message_id = uuid.new()
        trace = traces.new()

        -- Add the new empty assistant node as a child of parent_message_id.
        value.messages[new_message_id] = {
            id = new_message_id,
            parent_id = parent_message_id,
            role = "assistant",
            content = {},
            trace_id = trace.id,
        }

        -- Build the LLM trace from root → parent_message_id.
        local path = chat.get_path(value.messages, parent_message_id)
        for i, message in ipairs(path) do
            if message.role and message.content then
                trace[i] = { role = message.role, content = table.concat(message.content) }
            end
        end

        generate_handle = trace:generate("anthropic")

        -- TODO: Mark message as generating so it can't be branched off or affected while generating

        return value
    end)

    -- Stream chunks from the LLM into the chat state. Each chunk triggers a
    -- small JSON Patch (array-element add) rather than replacing the whole
    -- content string, keeping per-chunk SSE payloads small.
    for chunk in generate_handle do
        --- @cast chunk TraceGenerateChunk

        chat_state:update(function(value)
            --- @cast value Chat

            local message = value.messages[new_message_id]

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
    if final_value then
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
    -- TODO: Normalize list of message chunks into a single string in the list.
    return fs.load_json(string.format("chats/%s.json", chat_id))
end

--- @private
function chat.save(chat_state_contents)
    assert(chat_state_contents.id, "Tried to save chat content that has no id")
    fs.save_json(string.format("chats/%s.json", chat_state_contents.id), chat_state_contents)
end

return chat
