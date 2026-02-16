local arg = ...

package.loaded["chat"] = nil
package.loaded["util"] = nil
local chat = require("chat")

local dispatch = {
    -- Return:
    -- List of chat IDs (UUIDs as hyphenated strings)
    list = function()
        return chat.list_chats()
    end,

    -- Return:
    -- Chat ID (UUID as hyphenated string)
    new = function()
        local c = chat.new()
        return c.id
    end,

    -- Parameters:
    -- - chat_id
    -- Return:
    -- Chat object (see chat.lua)
    get = function(arg)
        return chat.get(arg.chat_id)
    end,

    -- Parameters:
    -- - chat_id
    -- - position: message index
    -- - role (optional)
    -- - content (optional)
    set_message = function(arg)
        chat.set_message(arg.chat_id, arg.position, arg.role, arg.content)
    end,

    -- Parameters:
    -- - chat_id
    generate = function(arg)
        chat.generate(arg.chat_id, "anthropic")
    end,
}

if not dispatch[arg.action] then
    error(string.format("Invalid action: '%s'", arg.action))
end

return dispatch[arg.action](arg)
