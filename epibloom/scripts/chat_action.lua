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

    -- Parameters:
    -- - chat_id
    -- - role
    -- - content
    append_message = function(arg)
        print("chat_action.append_message", arg.chat_id)
        chat.set_message(arg.chat_id, nil, arg.role, arg.content)
    end,

    -- Parameters:
    -- - chat_id
    generate = function(arg)
        print("chat_action.generate", arg.chat_id)
        chat.generate(arg.chat_id, "anthropic")
    end,
}

if not dispatch[arg.action] then
    error(string.format("Invalid action: '%s'", arg.action))
end

return dispatch[arg.action](arg)
