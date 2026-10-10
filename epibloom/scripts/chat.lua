local llm = require("epibloom.llm")
local log = require("epibloom.log")
local state = require("epibloom.state")
local time = require("epibloom.time")
local uuid = require("epibloom.uuid")

local util = require("util")

local chat = {}

--[[
DESIGN

Each chat is stored in two state objects:

- `chat:<id>:persist` (persisted): the `Chat` itself - settings and messages.
- `chat:<id>` (not persisted): `ChatStatus`, ephemeral state like whether a
  response is currently being generated. Since it isn't persisted, it is
  automatically reset if the application exits mid-generation.

Anything that needs both objects locks the persistent object first and then the
status object, nested inside the persistent object's update function. This
order must always be followed to avoid deadlocks. Code that only touches the
status object (eg. `chat.cancel_generate()`) may lock it alone.

While a response is generating, all modifications to the chat are rejected,
except for the updates made by the generation itself.

Each message index holds a `MessageSlot`, which has one or more alternates
("swipes") and the index of the currently selected one. Only the selected
alternate of each slot is sent to the LLM.

All indices (in function arguments and in the stored JSON) are 1-based.
--]]

chat.DEFAULT_MODEL = "claude-haiku-4-5"
chat.DEFAULT_MAX_TOKENS = 1024

--- @alias ChatId uuid

--- The top-level structure of a chat JSON object that is stored in a state object.
--- @class Chat
--- @field id ChatId
--- @field name string
--- @field system string # System prompt. Not sent if empty.
--- @field model string
--- @field max_tokens integer
--- @field messages MessageSlot[]
--- @field created string
--- @field modified string

--- Ephemeral (non-persisted) state for a chat.
--- @class ChatStatus
--- @field generating boolean # `true` while a response is being generated.
--- @field cancel_requested boolean # Set by `chat.cancel_generate()`, checked by the generation loop.
--- @field message_index? integer # While generating: the message being generated.
--- @field alternate_index? integer # While generating: the alternate being generated.
--- @field trace? string # While generating: the trace state object name.

--- A list of message alternates/rerolls ("swipes").
--- @class MessageSlot
--- @field alternates Message[] # Always non-empty.
--- @field selected integer # Index into `alternates` of the currently selected alternate.

--- A single message.
--- @class Message
--- @field role Role # Who this message is attributed to.
--- @field content MessageContent[]
--- @field note string # A user note attached to this message. Not sent to the LLM.
--- @field trace? string # Name of the trace state object for the LLM generation that produced this message.
--- @field error? string # Set if generation of this message failed or was cancelled.
--- @field created string
--- @field modified string

--- @class MessageInput
--- @field role Role
--- @field content? MessageContent[] # Defaults to empty.
--- @field note? string # Defaults to empty.

--- @alias MessageContent
--- | string[] # Text. This is a list for efficient chunk streaming via JSON Patch, the list elements should be concatenated to form the message text.

--- @alias Role
--- | "user"
--- | "assistant"

-- ============================================================================
-- Public API
-- ============================================================================

--- List the IDs of all stored chats.
--- @return ChatId[]
function chat.list()
    local ids = {}
    for _, name in ipairs(state.get(""):read()) do
        local id = string.match(name, "^chat:(.+):persist$")
        if id then
            table.insert(ids, id)
        end
    end
    return ids
end

--- Get a chat and its status.
--- @param chat_id ChatId
--- @return { chat: Chat, status: ChatStatus }
function chat.get(chat_id)
    chat._assert_exists(chat_id)
    return {
        chat = chat._persistent_object(chat_id):read(),
        status = chat._status_object(chat_id):read() or chat._default_status(),
    }
end

--- Create a new chat.
--- @param name? string
--- @return ChatId
function chat.new(name)
    local chat_id = uuid.now_v7()
    local now = time.now_utc_rfc3339()

    --- @type Chat
    local new_chat = {
        id = chat_id,
        name = name or now,
        system = "",
        model = chat.DEFAULT_MODEL,
        max_tokens = chat.DEFAULT_MAX_TOKENS,

        messages = {},

        created = now,
        modified = now,
    }

    chat._persistent_object(chat_id):update(function()
        return new_chat
    end)
    chat._status_object(chat_id):update(function()
        return chat._default_status()
    end)

    return chat_id
end

--- @param chat_id ChatId
--- @param name string
function chat.set_name(chat_id, name)
    assert(type(name) == "string", "name must be a string")
    chat._update(chat_id, function(c)
        c.name = name
    end)
end

--- Set the system prompt.
--- @param chat_id ChatId
--- @param system string
function chat.set_system(chat_id, system)
    assert(type(system) == "string", "system must be a string")
    chat._update(chat_id, function(c)
        c.system = system
    end)
end

--- Set generation settings. Fields that are not given are left unchanged.
--- @param chat_id ChatId
--- @param settings { model?: string, max_tokens?: integer }
function chat.set_settings(chat_id, settings)
    chat._update(chat_id, function(c)
        if settings.model ~= nil then
            assert(type(settings.model) == "string", "model must be a string")
            c.model = settings.model
        end
        if settings.max_tokens ~= nil then
            c.max_tokens = assert(math.tointeger(settings.max_tokens), "max_tokens must be an integer")
        end
    end)
end

--- Insert a new message slot (with a single alternate) at the given index,
--- shifting later messages up. Use an index one past the end to append.
--- @param chat_id ChatId
--- @param message_index integer
--- @param input MessageInput
--- @return integer message_index
function chat.insert_message(chat_id, message_index, input)
    local message = chat._new_message(input)

    return chat._update(chat_id, function(c)
        message_index = chat._check_index("message", chat_id, message_index, #c.messages + 1)
        table.insert(c.messages, message_index, { alternates = { message }, selected = 1 })
        return message_index
    end)
end

--- Append a new message slot to the end of the chat.
--- @param chat_id ChatId
--- @param input MessageInput
--- @return integer message_index
function chat.append_message(chat_id, input)
    local message = chat._new_message(input)

    return chat._update(chat_id, function(c)
        table.insert(c.messages, { alternates = { message }, selected = 1 })
        return #c.messages
    end)
end

--- Insert a new alternate into an existing message slot and select it.
--- @param chat_id ChatId
--- @param message_index integer
--- @param input MessageInput
--- @param alternate_index? integer # Defaults to appending after the last alternate.
--- @return integer alternate_index
function chat.insert_alternate(chat_id, message_index, input, alternate_index)
    local message = chat._new_message(input)

    return chat._update(chat_id, function(c)
        message_index = chat._check_index("message", chat_id, message_index, #c.messages)
        local slot = c.messages[message_index]

        alternate_index = alternate_index or #slot.alternates + 1
        alternate_index = chat._check_index("alternate", chat_id, alternate_index, #slot.alternates + 1)

        table.insert(slot.alternates, alternate_index, message)
        slot.selected = alternate_index
        return alternate_index
    end)
end

--- Set the content and/or note of an existing message alternate.
--- @param chat_id ChatId
--- @param message_index integer
--- @param alternate_index integer
--- @param content? MessageContent[] # If specified, overwrites the current content with this, otherwise it is left unchanged.
--- @param note? string # If specified, overwrites the current note with this, otherwise it is left unchanged.
function chat.set_message_content(chat_id, message_index, alternate_index, content, note)
    if content ~= nil then
        chat._check_content(content)
    end
    if note ~= nil then
        assert(type(note) == "string", "note must be a string")
    end

    chat._update(chat_id, function(c)
        local message = chat._get_alternate(chat_id, c, message_index, alternate_index)

        if content ~= nil then
            message.content = content
        end
        if note ~= nil then
            message.note = note
        end
        message.modified = time.now_utc_rfc3339()
    end)
end

--- Select which alternate of a message slot is active.
--- @param chat_id ChatId
--- @param message_index integer
--- @param alternate_index integer
function chat.select_alternate(chat_id, message_index, alternate_index)
    chat._update(chat_id, function(c)
        message_index = chat._check_index("message", chat_id, message_index, #c.messages)
        local slot = c.messages[message_index]
        slot.selected = chat._check_index("alternate", chat_id, alternate_index, #slot.alternates)
    end)
end

--- Delete a message alternate. If it was the only alternate, the whole
--- message slot is removed.
--- @param chat_id ChatId
--- @param message_index integer
--- @param alternate_index integer
function chat.delete_alternate(chat_id, message_index, alternate_index)
    chat._update(chat_id, function(c)
        message_index = chat._check_index("message", chat_id, message_index, #c.messages)
        local slot = c.messages[message_index]
        alternate_index = chat._check_index("alternate", chat_id, alternate_index, #slot.alternates)

        table.remove(slot.alternates, alternate_index)

        if #slot.alternates == 0 then
            table.remove(c.messages, message_index)
        elseif slot.selected > alternate_index or slot.selected > #slot.alternates then
            slot.selected = slot.selected - 1
        end
    end)
end

--- Generate a response.
---
--- If the last message is a user message, a new assistant message is appended.
--- If the last message is an assistant message, a new alternate of it is
--- generated (and selected).
---
--- The response is streamed into the chat as it arrives. This function
--- returns when generation is complete. To cancel generation, call
--- `chat.cancel_generate()` from a *different* script context (since this one
--- is busy until generation finishes).
--- @param chat_id ChatId
--- @return { message_index: integer, alternate_index: integer, trace: string, cancelled: boolean }
function chat.generate(chat_id)
    --- @type AnthropicGenerateStreamingHandle
    local handle
    local message_index, alternate_index

    -- Reserve the message we're generating and mark the chat as generating.
    chat._update(chat_id, function(c, status)
        local last_slot = c.messages[#c.messages]
        if not last_slot then
            util.errorf("Chat %s has no messages to respond to.", chat_id)
        end

        local last_message = last_slot.alternates[last_slot.selected]
        local context_length

        if last_message.role == "user" then
            context_length = #c.messages
            message_index = #c.messages + 1
            alternate_index = 1
        elseif last_message.role == "assistant" then
            context_length = #c.messages - 1
            message_index = #c.messages
            alternate_index = #last_slot.alternates + 1
        else
            util.errorf("Last message in chat %s has unknown role '%s'.", chat_id, tostring(last_message.role))
        end

        -- Start generation inside the lock so the trace name can be recorded
        -- atomically with the new message. If anything after this fails, the
        -- whole update is rolled back.
        handle = llm.anthropic_generate_streaming(chat._build_request(c, context_length))

        local message = chat._new_message { role = "assistant" }
        message.trace = handle.name

        if alternate_index == 1 then
            c.messages[message_index] = { alternates = { message }, selected = 1 }
        else
            last_slot.alternates[alternate_index] = message
            last_slot.selected = alternate_index
        end

        status.generating = true
        status.cancel_requested = false
        status.message_index = message_index
        status.alternate_index = alternate_index
        status.trace = handle.name
    end)

    local ok, result = pcall(chat._stream_response, chat_id, handle, message_index, alternate_index)
    local cancelled = ok and result

    if not ok then
        log.error("Generation for chat %s failed: %s", chat_id, tostring(result))
        handle:cancel()
    end

    -- Finalise the message and clear generation status.
    chat._update(chat_id, function(c, status)
        local message = c.messages[message_index].alternates[alternate_index]
        message.modified = time.now_utc_rfc3339()

        if not ok then
            message.error = tostring(result)
        elseif cancelled then
            message.error = "Generation was cancelled."
        end

        chat._reset_status(status)
    end, { allow_while_generating = true })

    if not ok then
        error(result, 0)
    end

    return {
        message_index = message_index,
        alternate_index = alternate_index,
        trace = handle.name,
        cancelled = cancelled,
    }
end

--- Request cancellation of an in-progress generation.
---
--- Must be called from a different script context than the one running
--- `chat.generate()`.
--- @param chat_id ChatId
--- @return boolean # `true` if a generation was in progress.
function chat.cancel_generate(chat_id)
    chat._assert_exists(chat_id)

    local was_generating = false
    chat._status_object(chat_id):update(function(status)
        status = status or chat._default_status()
        if status.generating then
            status.cancel_requested = true
            was_generating = true
        end
        return status
    end)

    return was_generating
end

-- ============================================================================
-- Internals
-- ============================================================================

--- Stream the response from `handle` into the given message.
--- @param chat_id ChatId
--- @param handle AnthropicGenerateStreamingHandle
--- @param message_index integer
--- @param alternate_index integer
--- @return boolean cancelled
--- @private
function chat._stream_response(chat_id, handle, message_index, alternate_index)
    local status_object = chat._status_object(chat_id)

    -- Maps API content block index (0-based) to our content index. Only text
    -- blocks are currently stored.
    local block_map = {}

    --- @param fn fun(message: Message)
    local function update_message(fn)
        chat._update(chat_id, function(c)
            fn(c.messages[message_index].alternates[alternate_index])
        end, { allow_while_generating = true, touch = false })
    end

    for ok, event in handle do
        if not ok then
            error(event, 0)
        end

        local status = status_object:read()
        if status and status.cancel_requested then
            handle:cancel()
            return true
        end

        if event.type == "content_block_start" and event.content_block.type == "text" then
            local text = event.content_block.text
            update_message(function(message)
                table.insert(message.content, (text and text ~= "") and { text } or {})
                block_map[event.index] = #message.content
            end)
        elseif event.type == "content_block_delta" and event.delta.type == "text_delta" then
            local content_index = block_map[event.index]
            if content_index then
                update_message(function(message)
                    table.insert(message.content[content_index], event.delta.text)
                end)
            end
        end
    end

    return false
end

--- Build an LLM request from the selected alternates of the first
--- `context_length` messages.
--- @param c Chat
--- @param context_length integer
--- @return anthropic.MessagesRequest
--- @private
function chat._build_request(c, context_length)
    local messages = {}

    for i = 1, context_length do
        local slot = c.messages[i]
        local message = slot.alternates[slot.selected]

        local blocks = {}
        for _, content in ipairs(message.content) do
            local text = table.concat(content)
            if text ~= "" then
                table.insert(blocks, { type = "text", text = text })
            end
        end

        -- The API rejects messages with no content.
        if #blocks > 0 then
            table.insert(messages, { role = message.role, content = blocks })
        end
    end

    --- @type anthropic.MessagesRequest
    local request = {
        model = c.model,
        max_tokens = c.max_tokens,
        messages = messages,
    }

    if c.system ~= nil and c.system ~= "" then
        request.system = c.system
    end

    return request
end

--- Atomically modify a chat and its status.
---
--- Locks the persistent object, then the status object. `fn` should modify the
--- chat and status tables in place, and can return a value which is returned
--- from this function. If `fn` raises an error, neither object is changed.
--- @param chat_id ChatId
--- @param fn fun(c: Chat, status: ChatStatus): any
--- @param opts? { allow_while_generating?: boolean, touch?: boolean } # `touch` (default `true`) updates the chat's modified time.
--- @return any
--- @private
function chat._update(chat_id, fn, opts)
    opts = opts or {}
    chat._assert_exists(chat_id)

    local result
    chat._persistent_object(chat_id):update(function(c)
        chat._status_object(chat_id):update(function(status)
            status = status or chat._default_status()

            if status.generating and not opts.allow_while_generating then
                util.errorf("Chat %s is currently generating a response and cannot be modified.", chat_id)
            end

            result = fn(c, status)
            return status
        end)

        if opts.touch ~= false then
            c.modified = time.now_utc_rfc3339()
        end

        return c
    end)

    return result
end

--- @param chat_id ChatId
--- @private
function chat._assert_exists(chat_id)
    if type(chat_id) ~= "string" or not util.table.find(chat.list(), chat_id) then
        util.errorf("No existing chat with ID '%s' found.", tostring(chat_id))
    end
end

--- @return ChatStatus
--- @private
function chat._default_status()
    return { generating = false, cancel_requested = false }
end

--- @param status ChatStatus
--- @private
function chat._reset_status(status)
    status.generating = false
    status.cancel_requested = false
    status.message_index = nil
    status.alternate_index = nil
    status.trace = nil
end

--- Validate a 1-based index is in `[1, max]`.
--- @return integer
--- @private
function chat._check_index(what, chat_id, index, max)
    local i = math.tointeger(index)
    if not i or i < 1 or i > max then
        util.errorf("Invalid %s index %s for chat %s (must be between 1 and %d).", what, tostring(index), chat_id, max)
    end
    return i
end

--- @param c Chat
--- @return Message
--- @private
function chat._get_alternate(chat_id, c, message_index, alternate_index)
    message_index = chat._check_index("message", chat_id, message_index, #c.messages)
    local slot = c.messages[message_index]
    alternate_index = chat._check_index("alternate", chat_id, alternate_index, #slot.alternates)
    return slot.alternates[alternate_index]
end

--- @param content any
--- @private
function chat._check_content(content)
    assert(type(content) == "table", "content must be a list of content blocks")
    for _, block in ipairs(content) do
        assert(type(block) == "table", "each content block must be a list of strings")
        for _, chunk in ipairs(block) do
            assert(type(chunk) == "string", "each content block must be a list of strings")
        end
    end
end

--- @param input MessageInput
--- @return Message
--- @private
function chat._new_message(input)
    assert(type(input) == "table", "message input must be a table")
    assert(
        input.role == "user" or input.role == "assistant",
        string.format("message role must be 'user' or 'assistant', got '%s'", tostring(input.role))
    )

    local content = input.content or {}
    chat._check_content(content)

    local note = input.note or ""
    assert(type(note) == "string", "note must be a string")

    local now = time.now_utc_rfc3339()

    return {
        role = input.role,
        content = content,
        note = note,
        created = now,
        modified = now,
    }
end

--- Get the persisted state object holding the `Chat`.
--- @param chat_id ChatId
--- @return StateObject
--- @private
function chat._persistent_object(chat_id)
    return state.get(string.format("chat:%s:persist", chat_id))
end

--- Get the non-persisted state object holding the `ChatStatus`.
--- @param chat_id ChatId
--- @return StateObject
--- @private
function chat._status_object(chat_id)
    return state.get(string.format("chat:%s", chat_id))
end

return chat
