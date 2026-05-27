local fs = require("epibloom.filesystem")
local log = require("epibloom.log")
local state = require("epibloom.state")
local llm = require("epibloom.llm")
local uuid = require("epibloom.uuid")
local util = require("util")

-- ---------------------------------------------------------------------------
-- Types
-- ---------------------------------------------------------------------------

local record ChatMessage
    --- Unique node identifier (UUIDv7).
    id: string
    --- Parent message ID; nil only for root nodes.
    parent_id: string | nil
    --- Who authored this message.
    role: llm.Role
    --- Thinking text as a list of string chunks, if present.
    thinking: {string} | nil
    --- Message content as a list of string chunks.
    content: {string}
    --- The trace that was used to generate this message, if applicable.
    trace_id: string | nil
    --- If an error occurred during generation, this holds the error message.
    error: string | nil
end

local record Chat
    id: string
    messages: {string: ChatMessage}
end

-- ---------------------------------------------------------------------------
-- Module
-- ---------------------------------------------------------------------------

local record ChatModule
    new: function(): Chat
    get: function(chat_id: string, create_if_not_found?: boolean): Chat
    poke: function(chat_id: string)
    list_chats: function(): {string}
    add_message: function(chat_id: string, parent_id: string | nil, role: llm.Role, content: string | {string} | nil): string
    get_path: function(messages: {string: ChatMessage}, node_id: string): {ChatMessage}
    generate: function(chat_id: string, parent_message_id: string)
    get_state: function(chat_id: string, create_if_not_found?: boolean): state.StateObject
    load: function(chat_id: string): Chat
    save: function(chat_state_contents: Chat)
end

local chat: ChatModule = {}

-- ---------------------------------------------------------------------------
-- Public API
-- ---------------------------------------------------------------------------

--- Create a new chat and return its initial state.
function chat.new(): Chat
    local new_chat_id = uuid.new()
    local c = chat.get(new_chat_id, true)
    chat.save(c)
    return c
end

--- Get the current state of a chat.
--- If create_if_not_found is false (default) and the chat does not exist, an error is raised.
function chat.get(chat_id: string, create_if_not_found?: boolean): Chat
    local chat_state = chat.get_state(chat_id, create_if_not_found)
    return chat_state:read() as Chat
end

--- Ensure a chat is loaded into the state object system (without returning it).
function chat.poke(chat_id: string)
    chat.get_state(chat_id)
end

--- Get a list of IDs for all existing chats.
function chat.list_chats(): {string}
    local list: {string} = {}

    for _, file in ipairs(fs.files("chats/*.json")) do
        local id = file:match("chats/([0-9a-f%-]+).json")
        if id then
            list[#list + 1] = id
        end
    end

    return list
end

--- Add a new message node to the chat tree and return its ID.
function chat.add_message(chat_id: string, parent_id: string | nil, role: llm.Role, content: string | {string} | nil): string
    local chat_state = chat.get_state(chat_id)

    -- NOTE: UUIDv7 is time-orderable, which lets us record the order of
    -- multiple branch messages coming off a single parent.
    local new_id = uuid.new()

    chat_state:update(function(value: any): any
        local chat_value = value as Chat

        local new_content: {string}
        if content then
            if content is string then
                new_content = { content }
            else
                new_content = content
            end
        else
            new_content = {}
        end

        chat_value.messages[new_id] = {
            id = new_id,
            parent_id = parent_id,
            role = role,
            content = new_content,
        }

        chat.save(chat_value)
        return chat_value
    end)

    return new_id
end

--- Walk parent_id links from node_id back to the root and return a
--- root-first ordered array of nodes.
function chat.get_path(messages: {string: ChatMessage}, node_id: string): {ChatMessage}
    local path: {ChatMessage} = {}
    local current_id: string | nil = node_id

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
function chat.generate(chat_id: string, parent_message_id: string)
    local chat_state = chat.get_state(chat_id)

    -- Read the current state to build the message path.
    local current_value = chat_state:read() as Chat
    local path = chat.get_path(current_value.messages, parent_message_id)

    -- Flatten each node's content chunks into a single text block, with a
    -- cache_control breakpoint so Anthropic will cache the prompt up to each
    -- message in the conversation history.
    local messages: {llm.Message} = {}
    for _, message in ipairs(path) do
        if message.role and message.content then
            messages[#messages + 1] = {
                role = message.role,
                content = {
                    {
                        type = "text",
                        text = table.concat(message.content),
                    }
                },
            }
        end
    end

    -- TODO: Set cache_control on the last content block for prompt caching.

    -- Start generation. Creates an llm-trace state object and begins streaming.
    local handle = llm.generate(messages, "anthropic")
    local new_message_id = uuid.new()

    -- Add the new empty assistant node to the chat state, including the trace
    -- ID so the UI can link to it.
    chat_state:update(function(value: any): any
        local chat_value = value as Chat
        chat_value.messages[new_message_id] = {
            id = new_message_id,
            parent_id = parent_message_id,
            role = "assistant",
            content = {},
            trace_id = handle.trace_id,
        }
        return chat_value
    end)

    -- Stream chunks from the LLM into the chat state. Each chunk triggers a
    -- small JSON Patch (array-element add) rather than replacing the whole
    -- content string, keeping per-chunk SSE payloads small.
    local chunk = handle()
    while chunk ~= nil do
        local c = chunk as llm.GenerateChunk
        chat_state:update(function(value: any): any
            local chat_value = value as Chat
            local message = chat_value.messages[new_message_id]

            if c.type == "message" then
                message.content[#message.content + 1] = c.delta
            elseif c.type == "thinking" then
                if not message.thinking then
                    message.thinking = {}
                end
                local thinking = message.thinking as {string}
                thinking[#thinking + 1] = c.delta
            elseif c.type == "error" then
                message.error = c.message
                log.error("Error in generation: %s", c.message)
            end

            return chat_value
        end)
        chunk = handle()
    end

    -- Persist to disk once, after the full response has been received.
    local final_value = chat_state:read()
    if final_value then
        chat.save(final_value as Chat)
    end
end

-- ---------------------------------------------------------------------------
-- Private helpers
-- ---------------------------------------------------------------------------

--- Get the state object associated with a chat, loading it from disk if needed.
--- If create_if_not_found is true the chat is created; otherwise an error is raised.
function chat.get_state(chat_id: string, create_if_not_found?: boolean): state.StateObject
    local chats = chat.list_chats()
    local state_name = "chat:" .. chat_id

    if util.table.find(chats, chat_id) then
        local chat_state = state.get(state_name)

        chat_state:update(function(value: any): any
            if value then
                return value
            end
            return chat.load(chat_id)
        end)

        return chat_state
    elseif create_if_not_found then
        local chat_state = state.get(state_name)

        chat_state:update(function(_: any): any
            return { id = chat_id, messages = {} }
        end)

        return chat_state
    else
        error("No chat exists with ID '" .. chat_id .. "'")
    end
end

function chat.load(chat_id: string): Chat
    -- TODO: Normalise list of message chunks into a single string in the list.
    return fs.load_json(string.format("chats/%s.json", chat_id)) as Chat
end

function chat.save(chat_state_contents: Chat)
    assert(chat_state_contents.id, "Tried to save chat content that has no id")
    fs.save_json(string.format("chats/%s.json", chat_state_contents.id), chat_state_contents)
end

return chat
