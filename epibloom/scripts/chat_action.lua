local arg = ...

local chattree = require("chattree")

local dispatch = {
    new = function()
        local chat = chattree.new()
        chat:save()
        return chat.id
    end,

    get = function(arg)
        print("Loading chat " .. arg.chat_id)
        local chat = chattree.load(arg.chat_id)
        return chat.chat
    end,
}

if not dispatch[arg.action] then
    error(string.format("Invalid action: '%s'", arg.action))
end

return dispatch[arg.action](arg)
