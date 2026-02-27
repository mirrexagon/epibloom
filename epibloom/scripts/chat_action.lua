local arg = ...

package.loaded["chat"] = nil
package.loaded["util"] = nil
local chat = require("chat")

local dispatch = {
    -- Return:
    -- List of chat IDs (UUIDs as hyphenated strings)
    list = function()
        print("chat_action.list")
        return chat.list_chats()
    end,

    -- Return:
    -- Chat ID (UUID as hyphenated string)
    new = function()
        print("chat_action.new")
        local c = chat.new()
        return c.id
    end,

    -- Make sure a chat is loaded into the state object system.
    -- Parameters:
    -- - chat_id
    poke = function(arg)
        chat.poke(arg.chat_id)
    end,

    -- Parameters:
    -- - chat_id
    -- Return:
    -- Chat object (see chat.lua)
    get = function(arg)
        print("chat_action.get", arg.chat_id)
        return chat.get(arg.chat_id)
    end,

    -- Add a user message as a child of the given parent node.
    -- Parameters:
    -- - chat_id
    -- - parent_id (string | null) -- the parent message ID, or null for the first message
    -- - content (string)
    -- Return:
    -- New message ID (UUID as hyphenated string)
    add_user_message = function(arg)
        print("chat_action.add_user_message", arg.chat_id)
        return chat.add_message(arg.chat_id, arg.parent_id, "user", arg.content)
    end,

    -- Generate an assistant response as a child of parent_message_id.
    -- Parameters:
    -- - chat_id
    -- - parent_message_id (string) -- the user-message node to respond to
    generate = function(arg)
        print("chat_action.generate", arg.chat_id)
        chat.generate(arg.chat_id, arg.parent_message_id)
    end,
}

if not dispatch[arg.action] then
    error(string.format("Invalid action: '%s'", arg.action))
end

return dispatch[arg.action](arg)
