-- Reload modules on every invocation so that edits to chat.tl or util.tl are
-- picked up without restarting the server.
package.loaded["chat"] = nil
package.loaded["util"] = nil

local chat = require("chat")

-- ---------------------------------------------------------------------------
-- Input type
-- ---------------------------------------------------------------------------

--- The JSON object passed in by the caller for every action.
--- Fields other than `action` are optional and action-dependent.
local record ChatActionInput
    action: string
    chat_id: string
    parent_id: string | nil
    parent_message_id: string | nil
    content: string | nil
end

local script_input: any = ...
local input = script_input as ChatActionInput

-- ---------------------------------------------------------------------------
-- Dispatch table
-- ---------------------------------------------------------------------------

local type DispatchFn = function(ChatActionInput): any

local dispatch: {string: DispatchFn} = {
    -- Return a list of chat IDs (UUIDs as hyphenated strings).
    list = function(_: ChatActionInput): any
        return chat.list_chats()
    end,

    -- Create a new chat and return its ID (UUID as hyphenated string).
    new = function(_: ChatActionInput): any
        local c = chat.new()
        return c.id
    end,

    -- Ensure a chat is loaded into the state object system.
    -- Parameters: chat_id
    poke = function(a: ChatActionInput): any
        chat.poke(a.chat_id)
    end,

    -- Return the Chat object for the given chat.
    -- Parameters: chat_id
    get = function(a: ChatActionInput): any
        return chat.get(a.chat_id)
    end,

    -- Add a user message as a child of the given parent node and return its ID.
    -- Parameters: chat_id, parent_id (string | null), content (string)
    add_user_message = function(a: ChatActionInput): any
        return chat.add_message(a.chat_id, a.parent_id, "user", a.content)
    end,

    -- Generate an assistant response as a child of parent_message_id.
    -- Parameters: chat_id, parent_message_id (string)
    generate = function(a: ChatActionInput): any
        chat.generate(a.chat_id, a.parent_message_id)
    end,
}

-- ---------------------------------------------------------------------------
-- Dispatch
-- ---------------------------------------------------------------------------

if not dispatch[input.action] then
    error(string.format("Invalid action: '%s'", input.action))
end

return dispatch[input.action](input)
